use std::collections::HashSet;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use twenty_first::prelude::bfe;
use twenty_first::prelude::x_field_element::EXTENSION_DEGREE;
use twenty_first::prelude::BFieldElement;
use twenty_first::prelude::XFieldElement;

use triton_vm::air::memory_layout;
use triton_vm::instruction::Instruction;
use triton_vm::op_stack::NumberOfWords;
use triton_vm::table::constraint_circuit::BinOp;
use triton_vm::table::constraint_circuit::CircuitExpression;
use triton_vm::table::constraint_circuit::ConstraintCircuit;
use triton_vm::table::constraint_circuit::InputIndicator;

use crate::codegen::Codegen;
use crate::codegen::TasmBackend;
use crate::constraints::Constraints;

/// An offset from the [memory layout][layout]'s `free_mem_page_ptr`, in number of
/// [`XFieldElement`]s. Indicates the start of the to-be-returned array.
///
/// [layout]: memory_layout::IntegralMemoryLayout
const OUT_ARRAY_OFFSET: usize = {
    let mem_page_size = memory_layout::MEM_PAGE_SIZE;
    let max_num_words_for_evaluated_constraints = 1 << 16; // magic!
    let out_array_offset_in_words = mem_page_size - max_num_words_for_evaluated_constraints;
    assert!(out_array_offset_in_words % EXTENSION_DEGREE == 0);
    out_array_offset_in_words / EXTENSION_DEGREE
};

/// Convenience macro to get raw opcodes of any [`Instruction`] variant, including its argument if
/// applicable.
///
/// [labelled]: triton_vm::instruction::LabelledInstruction::Instruction
macro_rules! instr {
    ($($instr:tt)*) => {{
        let instr = Instruction::$($instr)*;
        let opcode = u64::from(instr.opcode());
        match instr.arg().map(|arg| arg.value()) {
            Some(arg) => vec![quote!(#opcode), quote!(#arg)],
            None => vec![quote!(#opcode)],
        }
    }};
}

/// Convenience macro to get raw opcode of a [`Push`][push] instruction including its argument.
///
/// [push]: triton_vm::instruction::AnInstruction::Push
macro_rules! push {
    ($arg:ident) => {{
        let opcode = u64::from(Instruction::Push(BFieldElement::new(0)).opcode());
        let arg = u64::from($arg);
        vec![quote!(#opcode), quote!(#arg)]
    }};
    ($list:ident + $offset:expr) => {{
        let opcode = u64::from(Instruction::Push(BFieldElement::new(0)).opcode());
        let offset = u64::try_from($offset).unwrap();
        assert!(offset < u64::MAX - BFieldElement::P);
        // clippy will complain about the generated code if it contains `+ 0`
        if offset == 0 {
            vec![quote!(#opcode), quote!(#$list)]
        } else {
            vec![quote!(#opcode), quote!(#$list + #offset)]
        }
    }};
}

impl Codegen for TasmBackend {
    /// Emits a function that emits [Triton assembly][tasm] that evaluates Triton VM's AIR
    /// constraints over the [extension field][XFieldElement].
    ///
    /// [tasm]: triton_vm::prelude::triton_asm
    fn constraint_evaluation_code(constraints: &Constraints) -> TokenStream {
        let doc_comment = Self::doc_comment_static_version();

        let mut backend = Self::statically_known_input_locations();
        let init_constraints = backend.tokenize_circuits(&constraints.init());
        let cons_constraints = backend.tokenize_circuits(&constraints.cons());
        let tran_constraints = backend.tokenize_circuits(&constraints.tran());
        let term_constraints = backend.tokenize_circuits(&constraints.term());
        let prepare_return_values = Self::prepare_return_values();
        let num_instructions = init_constraints.len()
            + cons_constraints.len()
            + tran_constraints.len()
            + term_constraints.len()
            + prepare_return_values.len();
        let num_instructions = u64::try_from(num_instructions).unwrap();

        let convert_and_decode_assembled_instructions = quote!(
            let raw_instructions = raw_instructions
                .into_iter()
                .map(BFieldElement::new)
                .collect::<Vec<_>>();
            let program = Program::decode(&raw_instructions).unwrap();

            let irrelevant_label = |_: &_| String::new();
            program
                .into_iter()
                .map(|instruction| instruction.map_call_address(irrelevant_label))
                .map(LabelledInstruction::Instruction)
                .collect()
        );

        let statically_known_input_locations = quote!(
            #[doc = #doc_comment]
            pub fn static_air_constraint_evaluation_tasm(
                mem_layout: StaticTasmConstraintEvaluationMemoryLayout,
            ) -> Vec<LabelledInstruction> {
                let free_mem_page_ptr = mem_layout.free_mem_page_ptr.value();
                let curr_base_row_ptr = mem_layout.curr_base_row_ptr.value();
                let curr_ext_row_ptr = mem_layout.curr_ext_row_ptr.value();
                let next_base_row_ptr = mem_layout.next_base_row_ptr.value();
                let next_ext_row_ptr = mem_layout.next_ext_row_ptr.value();
                let challenges_ptr = mem_layout.challenges_ptr.value();

                let raw_instructions = vec![
                    #num_instructions,
                    #(#init_constraints,)*
                    #(#cons_constraints,)*
                    #(#tran_constraints,)*
                    #(#term_constraints,)*
                    #(#prepare_return_values,)*
                ];
                #convert_and_decode_assembled_instructions
            }
        );

        let doc_comment = Self::doc_comment_dynamic_version();

        let mut backend = Self::dynamically_known_input_locations();
        let move_row_pointers = backend.write_row_pointers_to_ram();
        let init_constraints = backend.tokenize_circuits(&constraints.init());
        let cons_constraints = backend.tokenize_circuits(&constraints.cons());
        let tran_constraints = backend.tokenize_circuits(&constraints.tran());
        let term_constraints = backend.tokenize_circuits(&constraints.term());
        let prepare_return_values = Self::prepare_return_values();
        let num_instructions = move_row_pointers.len()
            + init_constraints.len()
            + cons_constraints.len()
            + tran_constraints.len()
            + term_constraints.len()
            + prepare_return_values.len();
        let num_instructions = u64::try_from(num_instructions).unwrap();

        let dynamically_known_input_locations = quote!(
            #[doc = #doc_comment]
            pub fn dynamic_air_constraint_evaluation_tasm(
                mem_layout: DynamicTasmConstraintEvaluationMemoryLayout,
            ) -> Vec<LabelledInstruction> {
                let num_pointer_pointers = 4;
                let free_mem_page_ptr = mem_layout.free_mem_page_ptr.value() + num_pointer_pointers;
                let curr_base_row_ptr = mem_layout.free_mem_page_ptr.value();
                let curr_ext_row_ptr = mem_layout.free_mem_page_ptr.value() + 1;
                let next_base_row_ptr = mem_layout.free_mem_page_ptr.value() + 2;
                let next_ext_row_ptr = mem_layout.free_mem_page_ptr.value() + 3;
                let challenges_ptr = mem_layout.challenges_ptr.value();

                let raw_instructions = vec![
                    #num_instructions,
                    #(#move_row_pointers,)*
                    #(#init_constraints,)*
                    #(#cons_constraints,)*
                    #(#tran_constraints,)*
                    #(#term_constraints,)*
                    #(#prepare_return_values,)*
                ];
                #convert_and_decode_assembled_instructions
            }
        );

        let uses = Self::uses();
        quote!(
            #uses
            #statically_known_input_locations
            #dynamically_known_input_locations
        )
    }
}

impl TasmBackend {
    fn statically_known_input_locations() -> Self {
        Self {
            scope: HashSet::new(),
            elements_written: 0,
            input_location_is_static: true,
        }
    }

    fn dynamically_known_input_locations() -> Self {
        Self {
            input_location_is_static: false,
            ..Self::statically_known_input_locations()
        }
    }

    fn uses() -> TokenStream {
        quote!(
            use twenty_first::prelude::BFieldCodec;
            use twenty_first::prelude::BFieldElement;
            use crate::instruction::LabelledInstruction;
            use crate::Program;
            use crate::air::memory_layout::StaticTasmConstraintEvaluationMemoryLayout;
            use crate::air::memory_layout::DynamicTasmConstraintEvaluationMemoryLayout;
            // for rustdoc – https://github.com/rust-lang/rust/issues/74563
            #[allow(unused_imports)]
            use crate::table::extension_table::Quotientable;
        )
    }

    fn doc_comment_static_version() -> &'static str {
        "
         The emitted Triton assembly has the following signature:

         # Signature

         ```text
         BEFORE: _
         AFTER:  _ *evaluated_constraints
         ```
         # Requirements

         In order for this method to emit Triton assembly, various memory regions need to be
         declared. This is done through [`StaticTasmConstraintEvaluationMemoryLayout`]. The memory
         layout must be [integral].

         # Guarantees

         - The emitted code does not declare any labels.
         - The emitted code is “straight-line”, _i.e._, does not contain any of the instructions
           `call`, `return`, `recurse`, `recurse_or_return`, or `skiz`.
         - The emitted code does not contain instruction `halt`.
         - All memory write access of the emitted code is within the bounds of the memory region
           pointed to by `*free_memory_page`.
         - `*evaluated_constraints` points to an array of [`XFieldElement`][xfe]s of length
            [`NUM_CONSTRAINTS`][total]. Each element is the evaluation of one constraint. In
            particular, the disjoint sequence of slices containing
            [`NUM_INITIAL_CONSTRAINTS`][init], [`NUM_CONSISTENCY_CONSTRAINTS`][cons],
            [`NUM_TRANSITION_CONSTRAINTS`][tran], and [`NUM_TERMINAL_CONSTRAINTS`][term]
            (respectively and in this order) correspond to the evaluations of the initial,
            consistency, transition, and terminal constraints.

         [integral]: crate::air::memory_layout::IntegralMemoryLayout::is_integral
         [xfe]: twenty_first::prelude::XFieldElement
         [total]: crate::table::master_table::MasterExtTable::NUM_CONSTRAINTS
         [init]: crate::table::master_table::MasterExtTable::NUM_INITIAL_CONSTRAINTS
         [cons]: crate::table::master_table::MasterExtTable::NUM_CONSISTENCY_CONSTRAINTS
         [tran]: crate::table::master_table::MasterExtTable::NUM_TRANSITION_CONSTRAINTS
         [term]: crate::table::master_table::MasterExtTable::NUM_TERMINAL_CONSTRAINTS
        "
    }

    fn doc_comment_dynamic_version() -> &'static str {
        "
         The emitted Triton assembly has the following signature:

         # Signature

         ```text
         BEFORE: _ *current_main_row *current_aux_row *next_main_row *next_aux_row
         AFTER:  _ *evaluated_constraints
         ```
         # Requirements

         In order for this method to emit Triton assembly, various memory regions need to be
         declared. This is done through [`DynamicTasmConstraintEvaluationMemoryLayout`]. The memory
         layout must be [integral].

         # Guarantees

         - The emitted code does not declare any labels.
         - The emitted code is “straight-line”, _i.e._, does not contain any of the instructions
           `call`, `return`, `recurse`, `recurse_or_return`, or `skiz`.
         - The emitted code does not contain instruction `halt`.
         - All memory write access of the emitted code is within the bounds of the memory region
           pointed to by `*free_memory_page`.
         - `*evaluated_constraints` points to an array of [`XFieldElement`][xfe]s of length
            [`NUM_CONSTRAINTS`][total]. Each element is the evaluation of one constraint. In
            particular, the disjoint sequence of slices containing
            [`NUM_INITIAL_CONSTRAINTS`][init], [`NUM_CONSISTENCY_CONSTRAINTS`][cons],
            [`NUM_TRANSITION_CONSTRAINTS`][tran], and [`NUM_TERMINAL_CONSTRAINTS`][term]
            (respectively and in this order) correspond to the evaluations of the initial,
            consistency, transition, and terminal constraints.

         [integral]: crate::air::memory_layout::IntegralMemoryLayout::is_integral
         [xfe]: twenty_first::prelude::XFieldElement
         [total]: crate::table::master_table::MasterExtTable::NUM_CONSTRAINTS
         [init]: crate::table::master_table::MasterExtTable::NUM_INITIAL_CONSTRAINTS
         [cons]: crate::table::master_table::MasterExtTable::NUM_CONSISTENCY_CONSTRAINTS
         [tran]: crate::table::master_table::MasterExtTable::NUM_TRANSITION_CONSTRAINTS
         [term]: crate::table::master_table::MasterExtTable::NUM_TERMINAL_CONSTRAINTS
        "
    }

    /// Moves the dynamic arguments ({current, next} {main, aux} row pointers)
    /// to static addresses dedicated to them.
    fn write_row_pointers_to_ram(&self) -> Vec<TokenStream> {
        // BEFORE: _ *current_main_row *current_aux_row *next_main_row *next_aux_row
        // AFTER: _

        let write_pointer_to_ram = |list_id| {
            [
                push!(list_id + 0),
                instr!(WriteMem(NumberOfWords::N1)),
                instr!(Pop(NumberOfWords::N1)),
            ]
            .concat()
        };

        [
            IOList::NextExtRow,
            IOList::NextBaseRow,
            IOList::CurrExtRow,
            IOList::CurrBaseRow,
        ]
        .into_iter()
        .flat_map(write_pointer_to_ram)
        .collect()
    }

    fn tokenize_circuits<II: InputIndicator>(
        &mut self,
        constraints: &[ConstraintCircuit<II>],
    ) -> Vec<TokenStream> {
        self.scope = HashSet::new();
        let store_shared_nodes = self.store_all_shared_nodes(constraints);

        // to match the `RustBackend`, base constraints must be emitted first
        let (base_constraints, ext_constraints): (Vec<_>, Vec<_>) = constraints
            .iter()
            .partition(|constraint| constraint.evaluates_to_base_element());
        let sorted_constraints = base_constraints.into_iter().chain(ext_constraints);
        let write_to_output = sorted_constraints
            .map(|c| self.write_evaluated_constraint_into_output_list(c))
            .concat();

        [store_shared_nodes, write_to_output].concat()
    }

    fn store_all_shared_nodes<II: InputIndicator>(
        &mut self,
        constraints: &[ConstraintCircuit<II>],
    ) -> Vec<TokenStream> {
        let ref_counts = constraints.iter().flat_map(|c| c.all_ref_counters());
        let relevant_ref_counts = ref_counts.sorted().unique().filter(|&c| c > 1).rev();
        relevant_ref_counts
            .map(|count| self.store_all_shared_nodes_of_ref_count(constraints, count))
            .concat()
    }

    fn store_all_shared_nodes_of_ref_count<II: InputIndicator>(
        &mut self,
        constraints: &[ConstraintCircuit<II>],
        count: usize,
    ) -> Vec<TokenStream> {
        constraints
            .iter()
            .map(|c| self.store_single_shared_node_of_ref_count(c, count))
            .concat()
    }

    fn store_single_shared_node_of_ref_count<II: InputIndicator>(
        &mut self,
        constraint: &ConstraintCircuit<II>,
        ref_count: usize,
    ) -> Vec<TokenStream> {
        if self.scope.contains(&constraint.id) {
            return vec![];
        }

        let CircuitExpression::BinaryOperation(_, lhs, rhs) = &constraint.expression else {
            return vec![];
        };

        if constraint.ref_count < ref_count {
            let out_left = self.store_single_shared_node_of_ref_count(&lhs.borrow(), ref_count);
            let out_right = self.store_single_shared_node_of_ref_count(&rhs.borrow(), ref_count);
            return [out_left, out_right].concat();
        }

        assert_eq!(constraint.ref_count, ref_count);
        let evaluate = self.evaluate_single_node(constraint);
        let store = Self::store_ext_field_element(constraint.id);
        let is_new_insertion = self.scope.insert(constraint.id);
        assert!(is_new_insertion);

        [evaluate, store].concat()
    }

    fn evaluate_single_node<II: InputIndicator>(
        &self,
        constraint: &ConstraintCircuit<II>,
    ) -> Vec<TokenStream> {
        if self.scope.contains(&constraint.id) {
            return self.load_node(constraint);
        }

        let CircuitExpression::BinaryOperation(binop, lhs, rhs) = &constraint.expression else {
            return self.load_node(constraint);
        };

        let lhs = self.evaluate_single_node(&lhs.borrow());
        let rhs = self.evaluate_single_node(&rhs.borrow());
        let binop = match binop {
            BinOp::Add => instr!(XxAdd),
            BinOp::Mul => instr!(XxMul),
        };
        [lhs, rhs, binop].concat()
    }

    fn write_evaluated_constraint_into_output_list<II: InputIndicator>(
        &mut self,
        constraint: &ConstraintCircuit<II>,
    ) -> Vec<TokenStream> {
        let evaluated_constraint = self.evaluate_single_node(constraint);
        let element_index = OUT_ARRAY_OFFSET + self.elements_written;
        let store_element = Self::store_ext_field_element(element_index);
        self.elements_written += 1;
        [evaluated_constraint, store_element].concat()
    }

    fn load_node<II: InputIndicator>(&self, circuit: &ConstraintCircuit<II>) -> Vec<TokenStream> {
        match circuit.expression {
            CircuitExpression::BConstant(bfe) => Self::load_ext_field_constant(bfe.into()),
            CircuitExpression::XConstant(xfe) => Self::load_ext_field_constant(xfe),
            CircuitExpression::Input(input) => self.load_input(input),
            CircuitExpression::Challenge(challenge_idx) => Self::load_challenge(challenge_idx),
            CircuitExpression::BinaryOperation(_, _, _) => Self::load_evaluated_bin_op(circuit.id),
        }
    }

    fn load_ext_field_constant(xfe: XFieldElement) -> Vec<TokenStream> {
        let [c0, c1, c2] = xfe.coefficients.map(|c| push!(c));
        [c2, c1, c0].concat()
    }

    fn load_input<II: InputIndicator>(&self, input: II) -> Vec<TokenStream> {
        let list = match (input.is_current_row(), input.is_base_table_column()) {
            (true, true) => IOList::CurrBaseRow,
            (true, false) => IOList::CurrExtRow,
            (false, true) => IOList::NextBaseRow,
            (false, false) => IOList::NextExtRow,
        };
        if self.input_location_is_static {
            Self::load_ext_field_element_from_list(list, input.column())
        } else {
            Self::load_ext_field_element_from_pointed_to_list(list, input.column())
        }
    }

    fn load_challenge(challenge_idx: usize) -> Vec<TokenStream> {
        Self::load_ext_field_element_from_list(IOList::Challenges, challenge_idx)
    }

    fn load_evaluated_bin_op(node_id: usize) -> Vec<TokenStream> {
        Self::load_ext_field_element_from_list(IOList::FreeMemPage, node_id)
    }

    fn load_ext_field_element_from_list(list: IOList, element_index: usize) -> Vec<TokenStream> {
        let word_index = Self::element_index_to_word_index_for_reading(element_index);

        [
            push!(list + word_index),
            instr!(ReadMem(NumberOfWords::N3)),
            instr!(Pop(NumberOfWords::N1)),
        ]
        .concat()
    }

    fn load_ext_field_element_from_pointed_to_list(
        list: IOList,
        element_index: usize,
    ) -> Vec<TokenStream> {
        let word_index = Self::element_index_to_word_index_for_reading(element_index);

        [
            push!(list + 0),
            instr!(ReadMem(NumberOfWords::N1)),
            instr!(Pop(NumberOfWords::N1)),
            instr!(AddI(word_index)),
            instr!(ReadMem(NumberOfWords::N3)),
            instr!(Pop(NumberOfWords::N1)),
        ]
        .concat()
    }

    fn element_index_to_word_index_for_reading(element_index: usize) -> BFieldElement {
        let word_offset = element_index * EXTENSION_DEGREE;
        let start_to_read_offset = EXTENSION_DEGREE - 1;
        let word_index = word_offset + start_to_read_offset;
        bfe!(u64::try_from(word_index).unwrap())
    }

    fn store_ext_field_element(element_index: usize) -> Vec<TokenStream> {
        let free_mem_page = IOList::FreeMemPage;

        let word_offset = element_index * EXTENSION_DEGREE;
        let word_index = u64::try_from(word_offset).unwrap();

        let push_address = push!(free_mem_page + word_index);
        let write_mem = instr!(WriteMem(NumberOfWords::N3));
        let pop = instr!(Pop(NumberOfWords::N1));

        [push_address, write_mem, pop].concat()
    }

    fn prepare_return_values() -> Vec<TokenStream> {
        let free_mem_page = IOList::FreeMemPage;
        let out_array_offset_in_num_bfes = OUT_ARRAY_OFFSET * EXTENSION_DEGREE;
        let out_array_offset = u64::try_from(out_array_offset_in_num_bfes).unwrap();
        push!(free_mem_page + out_array_offset)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum IOList {
    FreeMemPage,
    CurrBaseRow,
    CurrExtRow,
    NextBaseRow,
    NextExtRow,
    Challenges,
}

impl ToTokens for IOList {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            IOList::FreeMemPage => tokens.extend(quote!(free_mem_page_ptr)),
            IOList::CurrBaseRow => tokens.extend(quote!(curr_base_row_ptr)),
            IOList::CurrExtRow => tokens.extend(quote!(curr_ext_row_ptr)),
            IOList::NextBaseRow => tokens.extend(quote!(next_base_row_ptr)),
            IOList::NextExtRow => tokens.extend(quote!(next_ext_row_ptr)),
            IOList::Challenges => tokens.extend(quote!(challenges_ptr)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::tests::print_constraints;

    use super::*;

    #[test]
    fn print_mini_constraints() {
        print_constraints::<TasmBackend>(&Constraints::mini_constraints());
    }

    #[test]
    fn print_test_constraints() {
        print_constraints::<TasmBackend>(&Constraints::test_constraints());
    }
}
