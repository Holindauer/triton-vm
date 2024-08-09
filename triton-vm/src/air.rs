pub mod memory_layout;
pub mod tasm_air_constraints;

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use ndarray::Array1;
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest_arbitrary_interop::arb;
    use std::collections::HashMap;
    use test_strategy::proptest;

    use crate::air::memory_layout::DynamicTasmConstraintEvaluationMemoryLayout;
    use crate::air::memory_layout::IntegralMemoryLayout;
    use crate::air::memory_layout::MemoryRegion;
    use crate::air::memory_layout::StaticTasmConstraintEvaluationMemoryLayout;
    use crate::air::tasm_air_constraints::dynamic_air_constraint_evaluation_tasm;
    use crate::air::tasm_air_constraints::static_air_constraint_evaluation_tasm;
    use crate::bfe;
    use crate::instruction::AnInstruction;
    use crate::instruction::LabelledInstruction;
    use crate::table::challenges::Challenges;
    use crate::table::extension_table::Evaluable;
    use crate::table::extension_table::Quotientable;
    use crate::table::master_table::MasterExtTable;
    use crate::table::NUM_BASE_COLUMNS;
    use crate::table::NUM_EXT_COLUMNS;
    use crate::triton_instr;
    use crate::twenty_first::prelude::x_field_element::EXTENSION_DEGREE;
    use crate::BFieldElement;
    use crate::NonDeterminism;
    use crate::Program;
    use crate::PublicInput;
    use crate::VMState;
    use crate::XFieldElement;

    #[derive(Debug, Clone, test_strategy::Arbitrary)]
    struct ConstraintEvaluationPoint {
        #[strategy(vec(arb(), NUM_BASE_COLUMNS))]
        #[map(Array1::from)]
        curr_base_row: Array1<XFieldElement>,

        #[strategy(vec(arb(), NUM_EXT_COLUMNS))]
        #[map(Array1::from)]
        curr_ext_row: Array1<XFieldElement>,

        #[strategy(vec(arb(), NUM_BASE_COLUMNS))]
        #[map(Array1::from)]
        next_base_row: Array1<XFieldElement>,

        #[strategy(vec(arb(), NUM_EXT_COLUMNS))]
        #[map(Array1::from)]
        next_ext_row: Array1<XFieldElement>,

        #[strategy(arb())]
        challenges: Challenges,

        #[strategy(arb())]
        #[filter(#static_memory_layout.is_integral())]
        static_memory_layout: StaticTasmConstraintEvaluationMemoryLayout,
    }

    impl ConstraintEvaluationPoint {
        fn evaluate_all_constraints_rust(&self) -> Vec<XFieldElement> {
            let init = MasterExtTable::evaluate_initial_constraints(
                self.curr_base_row.view(),
                self.curr_ext_row.view(),
                &self.challenges,
            );
            let cons = MasterExtTable::evaluate_consistency_constraints(
                self.curr_base_row.view(),
                self.curr_ext_row.view(),
                &self.challenges,
            );
            let tran = MasterExtTable::evaluate_transition_constraints(
                self.curr_base_row.view(),
                self.curr_ext_row.view(),
                self.next_base_row.view(),
                self.next_ext_row.view(),
                &self.challenges,
            );
            let term = MasterExtTable::evaluate_terminal_constraints(
                self.curr_base_row.view(),
                self.curr_ext_row.view(),
                &self.challenges,
            );

            [init, cons, tran, term].concat()
        }

        fn evaluate_all_constraints_tasm_static(&self) -> Vec<XFieldElement> {
            let program = self.tasm_static_constraint_evaluation_code();
            let mut vm_state =
                self.set_up_triton_vm_to_evaluate_constraints_in_tasm_static(&program);

            vm_state.run().unwrap();

            let output_list_ptr = vm_state.op_stack.pop().unwrap().value();
            let num_quotients = MasterExtTable::NUM_CONSTRAINTS;
            Self::read_xfe_list_at_address(vm_state.ram, output_list_ptr, num_quotients)
        }

        fn evaluate_all_constraints_tasm_dynamic(&self) -> Vec<XFieldElement> {
            let program = self.tasm_dynamic_constraint_evaluation_code();
            let mut vm_state =
                self.set_up_triton_vm_to_evaluate_constraints_in_tasm_dynamic(&program);

            vm_state.run().unwrap();

            let output_list_ptr = vm_state.op_stack.pop().unwrap().value();
            let num_quotients = MasterExtTable::NUM_CONSTRAINTS;
            Self::read_xfe_list_at_address(vm_state.ram, output_list_ptr, num_quotients)
        }

        fn tasm_static_constraint_evaluation_code(&self) -> Program {
            let mut source_code = static_air_constraint_evaluation_tasm(self.static_memory_layout);
            source_code.push(triton_instr!(halt));
            Program::new(&source_code)
        }

        fn tasm_dynamic_constraint_evaluation_code(&self) -> Program {
            let dynamic_memory_layout = DynamicTasmConstraintEvaluationMemoryLayout {
                free_mem_page_ptr: self.static_memory_layout.free_mem_page_ptr,
                challenges_ptr: self.static_memory_layout.challenges_ptr,
            };
            let mut source_code = dynamic_air_constraint_evaluation_tasm(dynamic_memory_layout);
            source_code.push(triton_instr!(halt));
            Program::new(&source_code)
        }

        fn set_up_triton_vm_to_evaluate_constraints_in_tasm_static(
            &self,
            program: &Program,
        ) -> VMState {
            let curr_base_row_ptr = self.static_memory_layout.curr_base_row_ptr;
            let curr_ext_row_ptr = self.static_memory_layout.curr_ext_row_ptr;
            let next_base_row_ptr = self.static_memory_layout.next_base_row_ptr;
            let next_ext_row_ptr = self.static_memory_layout.next_ext_row_ptr;
            let challenges_ptr = self.static_memory_layout.challenges_ptr;

            let mut ram = HashMap::default();
            Self::extend_ram_at_address(&mut ram, self.curr_base_row.to_vec(), curr_base_row_ptr);
            Self::extend_ram_at_address(&mut ram, self.curr_ext_row.to_vec(), curr_ext_row_ptr);
            Self::extend_ram_at_address(&mut ram, self.next_base_row.to_vec(), next_base_row_ptr);
            Self::extend_ram_at_address(&mut ram, self.next_ext_row.to_vec(), next_ext_row_ptr);
            Self::extend_ram_at_address(&mut ram, self.challenges.challenges, challenges_ptr);
            let non_determinism = NonDeterminism::default().with_ram(ram);

            VMState::new(program, PublicInput::default(), non_determinism)
        }

        fn set_up_triton_vm_to_evaluate_constraints_in_tasm_dynamic(
            &self,
            program: &Program,
        ) -> VMState {
            // for convenience, reuse the (integral) static memory layout
            let mut vm_state =
                self.set_up_triton_vm_to_evaluate_constraints_in_tasm_static(program);
            vm_state
                .op_stack
                .push(self.static_memory_layout.curr_base_row_ptr);
            vm_state
                .op_stack
                .push(self.static_memory_layout.curr_ext_row_ptr);
            vm_state
                .op_stack
                .push(self.static_memory_layout.next_base_row_ptr);
            vm_state
                .op_stack
                .push(self.static_memory_layout.next_ext_row_ptr);
            vm_state
        }

        fn extend_ram_at_address(
            ram: &mut HashMap<BFieldElement, BFieldElement>,
            list: impl IntoIterator<Item = impl Into<XFieldElement>>,
            address: BFieldElement,
        ) {
            let list = list.into_iter().flat_map(|xfe| xfe.into().coefficients);
            let indexed_list = list.enumerate();
            let offset_address = |i| bfe!(i as u64) + address;
            let ram_extension = indexed_list.map(|(i, bfe)| (offset_address(i), bfe));
            ram.extend(ram_extension);
        }

        fn read_xfe_list_at_address(
            ram: HashMap<BFieldElement, BFieldElement>,
            address: u64,
            len: usize,
        ) -> Vec<XFieldElement> {
            let mem_region_end = address + (len * EXTENSION_DEGREE) as u64;
            (address..mem_region_end)
                .map(BFieldElement::new)
                .map(|i| ram[&i])
                .chunks(EXTENSION_DEGREE)
                .into_iter()
                .map(|c| XFieldElement::try_from(c.collect_vec()).unwrap())
                .collect()
        }
    }

    #[proptest]
    fn triton_constraints_and_assembly_constraints_agree(point: ConstraintEvaluationPoint) {
        let all_constraints_rust = point.evaluate_all_constraints_rust();
        let all_constraints_tasm_static = point.evaluate_all_constraints_tasm_static();
        prop_assert_eq!(all_constraints_rust.clone(), all_constraints_tasm_static);

        let all_constraints_tasm_dynamic = point.evaluate_all_constraints_tasm_dynamic();
        prop_assert_eq!(all_constraints_rust, all_constraints_tasm_dynamic);
    }

    #[proptest]
    fn triton_assembly_constraint_evaluators_do_not_write_outside_of_dedicated_memory_region(
        point: ConstraintEvaluationPoint,
    ) {
        let program = point.tasm_static_constraint_evaluation_code();
        let mut initial_state =
            point.set_up_triton_vm_to_evaluate_constraints_in_tasm_static(&program);
        let mut terminal_state = initial_state.clone();
        terminal_state.run().unwrap();

        let free_mem_page_ptr = point.static_memory_layout.free_mem_page_ptr;
        let mem_page_size = StaticTasmConstraintEvaluationMemoryLayout::MEM_PAGE_SIZE;
        let mem_page = MemoryRegion::new(free_mem_page_ptr, mem_page_size);
        let not_in_mem_page = |addr: &_| !mem_page.contains_address(addr);

        initial_state.ram.retain(|k, _| not_in_mem_page(k));
        terminal_state.ram.retain(|k, _| not_in_mem_page(k));
        prop_assert_eq!(initial_state.ram, terminal_state.ram);
    }

    #[proptest]
    fn triton_assembly_constraint_evaluators_declare_no_labels(
        #[strategy(arb())] static_memory_layout: StaticTasmConstraintEvaluationMemoryLayout,
        #[strategy(arb())] dynamic_memory_layout: DynamicTasmConstraintEvaluationMemoryLayout,
    ) {
        for instruction in static_air_constraint_evaluation_tasm(static_memory_layout)
            .into_iter()
            .chain(dynamic_air_constraint_evaluation_tasm(
                dynamic_memory_layout,
            ))
        {
            if let LabelledInstruction::Label(label) = instruction {
                return Err(TestCaseError::Fail(format!("Found label: {label}").into()));
            }
        }
    }

    #[proptest]
    fn triton_assembly_constraint_evaluators_are_straight_line_and_does_not_halt(
        #[strategy(arb())] static_memory_layout: StaticTasmConstraintEvaluationMemoryLayout,
        #[strategy(arb())] dynamic_memory_layout: DynamicTasmConstraintEvaluationMemoryLayout,
    ) {
        type I = AnInstruction<String>;
        let is_legal = |i| !matches!(i, I::Call(_) | I::Return | I::Recurse | I::Skiz | I::Halt);

        for instruction in static_air_constraint_evaluation_tasm(static_memory_layout) {
            if let LabelledInstruction::Instruction(instruction) = instruction {
                prop_assert!(is_legal(instruction));
            }
        }

        for instruction in dynamic_air_constraint_evaluation_tasm(dynamic_memory_layout) {
            if let LabelledInstruction::Instruction(instruction) = instruction {
                prop_assert!(is_legal(instruction));
            }
        }
    }
}
