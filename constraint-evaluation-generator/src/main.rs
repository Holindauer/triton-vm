use std::collections::HashSet;
use std::process::Command;

use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::x_field_element::XFieldElement;

use triton_vm::table::cascade_table::ExtCascadeTable;
use triton_vm::table::constraint_circuit::BinOp;
use triton_vm::table::constraint_circuit::CircuitExpression;
use triton_vm::table::constraint_circuit::CircuitExpression::*;
use triton_vm::table::constraint_circuit::ConstraintCircuit;
use triton_vm::table::constraint_circuit::ConstraintCircuitBuilder;
use triton_vm::table::constraint_circuit::ConstraintCircuitMonad;
use triton_vm::table::constraint_circuit::DualRowIndicator;
use triton_vm::table::constraint_circuit::InputIndicator;
use triton_vm::table::constraint_circuit::SingleRowIndicator;
use triton_vm::table::cross_table_argument::GrandCrossTableArg;
use triton_vm::table::degree_lowering_table;
use triton_vm::table::hash_table::ExtHashTable;
use triton_vm::table::jump_stack_table::ExtJumpStackTable;
use triton_vm::table::lookup_table::ExtLookupTable;
use triton_vm::table::master_table;
use triton_vm::table::op_stack_table::ExtOpStackTable;
use triton_vm::table::processor_table::ExtProcessorTable;
use triton_vm::table::program_table::ExtProgramTable;
use triton_vm::table::ram_table::ExtRamTable;
use triton_vm::table::u32_table::ExtU32Table;

fn main() {
    let circuit_builder = ConstraintCircuitBuilder::new();
    let mut initial_constraints = vec![
        ExtProgramTable::initial_constraints(&circuit_builder),
        ExtProcessorTable::initial_constraints(&circuit_builder),
        ExtOpStackTable::initial_constraints(&circuit_builder),
        ExtRamTable::initial_constraints(&circuit_builder),
        ExtJumpStackTable::initial_constraints(&circuit_builder),
        ExtHashTable::initial_constraints(&circuit_builder),
        ExtCascadeTable::initial_constraints(&circuit_builder),
        ExtLookupTable::initial_constraints(&circuit_builder),
        ExtU32Table::initial_constraints(&circuit_builder),
        GrandCrossTableArg::initial_constraints(&circuit_builder),
    ]
    .concat();

    let circuit_builder = ConstraintCircuitBuilder::new();
    let mut consistency_constraints = vec![
        ExtProgramTable::consistency_constraints(&circuit_builder),
        ExtProcessorTable::consistency_constraints(&circuit_builder),
        ExtOpStackTable::consistency_constraints(&circuit_builder),
        ExtRamTable::consistency_constraints(&circuit_builder),
        ExtJumpStackTable::consistency_constraints(&circuit_builder),
        ExtHashTable::consistency_constraints(&circuit_builder),
        ExtCascadeTable::consistency_constraints(&circuit_builder),
        ExtLookupTable::consistency_constraints(&circuit_builder),
        ExtU32Table::consistency_constraints(&circuit_builder),
        GrandCrossTableArg::consistency_constraints(&circuit_builder),
    ]
    .concat();

    let circuit_builder = ConstraintCircuitBuilder::new();
    let mut transition_constraints = vec![
        ExtProgramTable::transition_constraints(&circuit_builder),
        ExtProcessorTable::transition_constraints(&circuit_builder),
        ExtOpStackTable::transition_constraints(&circuit_builder),
        ExtRamTable::transition_constraints(&circuit_builder),
        ExtJumpStackTable::transition_constraints(&circuit_builder),
        ExtHashTable::transition_constraints(&circuit_builder),
        ExtCascadeTable::transition_constraints(&circuit_builder),
        ExtLookupTable::transition_constraints(&circuit_builder),
        ExtU32Table::transition_constraints(&circuit_builder),
        GrandCrossTableArg::transition_constraints(&circuit_builder),
    ]
    .concat();

    let circuit_builder = ConstraintCircuitBuilder::new();
    let mut terminal_constraints = vec![
        ExtProgramTable::terminal_constraints(&circuit_builder),
        ExtProcessorTable::terminal_constraints(&circuit_builder),
        ExtOpStackTable::terminal_constraints(&circuit_builder),
        ExtRamTable::terminal_constraints(&circuit_builder),
        ExtJumpStackTable::terminal_constraints(&circuit_builder),
        ExtHashTable::terminal_constraints(&circuit_builder),
        ExtCascadeTable::terminal_constraints(&circuit_builder),
        ExtLookupTable::terminal_constraints(&circuit_builder),
        ExtU32Table::terminal_constraints(&circuit_builder),
        GrandCrossTableArg::terminal_constraints(&circuit_builder),
    ]
    .concat();

    ConstraintCircuitMonad::constant_folding(&mut initial_constraints);
    ConstraintCircuitMonad::constant_folding(&mut consistency_constraints);
    ConstraintCircuitMonad::constant_folding(&mut transition_constraints);
    ConstraintCircuitMonad::constant_folding(&mut terminal_constraints);

    // Subtract the degree lowering table's width from the total number of columns to guarantee
    // the same number of columns even for repeated runs of the constraint evaluation generator.
    let mut num_base_cols = master_table::NUM_BASE_COLUMNS - degree_lowering_table::BASE_WIDTH;
    let mut num_ext_cols = master_table::NUM_EXT_COLUMNS - degree_lowering_table::EXT_WIDTH;
    let (init_base_substitutions, init_ext_substitutions) = ConstraintCircuitMonad::lower_to_degree(
        &mut initial_constraints,
        master_table::AIR_TARGET_DEGREE,
        num_base_cols,
        num_ext_cols,
    );
    num_base_cols += init_base_substitutions.len();
    num_ext_cols += init_ext_substitutions.len();

    let (cons_base_substitutions, cons_ext_substitutions) = ConstraintCircuitMonad::lower_to_degree(
        &mut consistency_constraints,
        master_table::AIR_TARGET_DEGREE,
        num_base_cols,
        num_ext_cols,
    );
    num_base_cols += cons_base_substitutions.len();
    num_ext_cols += cons_ext_substitutions.len();

    let (tran_base_substitutions, tran_ext_substitutions) = ConstraintCircuitMonad::lower_to_degree(
        &mut transition_constraints,
        master_table::AIR_TARGET_DEGREE,
        num_base_cols,
        num_ext_cols,
    );
    num_base_cols += tran_base_substitutions.len();
    num_ext_cols += tran_ext_substitutions.len();

    let (term_base_substitutions, term_ext_substitutions) = ConstraintCircuitMonad::lower_to_degree(
        &mut terminal_constraints,
        master_table::AIR_TARGET_DEGREE,
        num_base_cols,
        num_ext_cols,
    );

    let table_code = generate_degree_lowering_table_code(
        &init_base_substitutions,
        &cons_base_substitutions,
        &tran_base_substitutions,
        &term_base_substitutions,
        &init_ext_substitutions,
        &cons_ext_substitutions,
        &tran_ext_substitutions,
        &term_ext_substitutions,
    );

    let initial_constraints = vec![
        initial_constraints,
        init_base_substitutions,
        init_ext_substitutions,
    ]
    .concat();
    let consistency_constraints = vec![
        consistency_constraints,
        cons_base_substitutions,
        cons_ext_substitutions,
    ]
    .concat();
    let transition_constraints = vec![
        transition_constraints,
        tran_base_substitutions,
        tran_ext_substitutions,
    ]
    .concat();
    let terminal_constraints = vec![
        terminal_constraints,
        term_base_substitutions,
        term_ext_substitutions,
    ]
    .concat();

    let mut initial_constraints = consume(initial_constraints);
    let mut consistency_constraints = consume(consistency_constraints);
    let mut transition_constraints = consume(transition_constraints);
    let mut terminal_constraints = consume(terminal_constraints);

    let constraint_code = generate_constraint_code(
        &mut initial_constraints,
        &mut consistency_constraints,
        &mut transition_constraints,
        &mut terminal_constraints,
    );

    let table_syntax_tree = syn::parse2(table_code).unwrap();
    let table_code = prettyplease::unparse(&table_syntax_tree);
    match std::fs::write("triton-vm/src/table/degree_lowering_table.rs", table_code) {
        Ok(_) => (),
        Err(err) => panic!("Writing to disk has failed: {err}"),
    }

    let constraint_syntax_tree = syn::parse2(constraint_code).unwrap();
    let constraint_code = prettyplease::unparse(&constraint_syntax_tree);
    match std::fs::write("triton-vm/src/table/constraints.rs", constraint_code) {
        Ok(_) => (),
        Err(err) => panic!("Writing to disk has failed: {err}"),
    }

    match Command::new("cargo")
        .arg("clippy")
        .arg("--workspace")
        .arg("--all-targets")
        .output()
    {
        Ok(_) => (),
        Err(err) => panic!("cargo clippy failed: {err}"),
    }
}

/// Consumes every `ConstraintCircuitMonad`, returning their corresponding `ConstraintCircuit`s.
fn consume<II: InputIndicator>(
    constraints: Vec<ConstraintCircuitMonad<II>>,
) -> Vec<ConstraintCircuit<II>> {
    constraints.into_iter().map(|c| c.consume()).collect()
}

fn generate_constraint_code<SII: InputIndicator, DII: InputIndicator>(
    init_constraint_circuits: &mut [ConstraintCircuit<SII>],
    cons_constraint_circuits: &mut [ConstraintCircuit<SII>],
    tran_constraint_circuits: &mut [ConstraintCircuit<DII>],
    term_constraint_circuits: &mut [ConstraintCircuit<SII>],
) -> TokenStream {
    let num_init_constraints = init_constraint_circuits.len();
    let num_cons_constraints = cons_constraint_circuits.len();
    let num_tran_constraints = tran_constraint_circuits.len();
    let num_term_constraints = term_constraint_circuits.len();

    let (init_constraint_degrees, init_constraints_bfe, init_constraints_xfe) =
        tokenize_circuits(init_constraint_circuits);
    let (cons_constraint_degrees, cons_constraints_bfe, cons_constraints_xfe) =
        tokenize_circuits(cons_constraint_circuits);
    let (tran_constraint_degrees, tran_constraints_bfe, tran_constraints_xfe) =
        tokenize_circuits(tran_constraint_circuits);
    let (term_constraint_degrees, term_constraints_bfe, term_constraints_xfe) =
        tokenize_circuits(term_constraint_circuits);

    quote!(
    use ndarray::ArrayView1;
    use twenty_first::shared_math::b_field_element::BFieldElement;
    use twenty_first::shared_math::mpolynomial::Degree;
    use twenty_first::shared_math::x_field_element::XFieldElement;

    use crate::table::challenges::Challenges;
    use crate::table::challenges::ChallengeId::*;
    use crate::table::extension_table::Evaluable;
    use crate::table::extension_table::Quotientable;
    use crate::table::master_table::MasterExtTable;

    // This file has been auto-generated. Any modifications _will_ be lost.
    // To re-generate, execute:
    // `cargo run --bin constraint-evaluation-generator`
    impl Evaluable<BFieldElement> for MasterExtTable {
        #[inline]
        #[allow(unused_variables)]
        fn evaluate_initial_constraints(
            base_row: ArrayView1<BFieldElement>,
            ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #init_constraints_bfe
        }

        #[inline]
        #[allow(unused_variables)]
        fn evaluate_consistency_constraints(
            base_row: ArrayView1<BFieldElement>,
            ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #cons_constraints_bfe
        }

        #[inline]
        #[allow(unused_variables)]
        fn evaluate_transition_constraints(
            current_base_row: ArrayView1<BFieldElement>,
            current_ext_row: ArrayView1<XFieldElement>,
            next_base_row: ArrayView1<BFieldElement>,
            next_ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #tran_constraints_bfe
        }

        #[inline]
        #[allow(unused_variables)]
        fn evaluate_terminal_constraints(
            base_row: ArrayView1<BFieldElement>,
            ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #term_constraints_bfe
        }
    }

    impl Evaluable<XFieldElement> for MasterExtTable {
        #[inline]
        #[allow(unused_variables)]
        fn evaluate_initial_constraints(
            base_row: ArrayView1<XFieldElement>,
            ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #init_constraints_xfe
        }

        #[inline]
        #[allow(unused_variables)]
        fn evaluate_consistency_constraints(
            base_row: ArrayView1<XFieldElement>,
            ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #cons_constraints_xfe
        }

        #[inline]
        #[allow(unused_variables)]
        fn evaluate_transition_constraints(
            current_base_row: ArrayView1<XFieldElement>,
            current_ext_row: ArrayView1<XFieldElement>,
            next_base_row: ArrayView1<XFieldElement>,
            next_ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #tran_constraints_xfe
        }

        #[inline]
        #[allow(unused_variables)]
        fn evaluate_terminal_constraints(
            base_row: ArrayView1<XFieldElement>,
            ext_row: ArrayView1<XFieldElement>,
            challenges: &Challenges,
        ) -> Vec<XFieldElement> {
            #term_constraints_xfe
        }
    }

    impl Quotientable for MasterExtTable {
        fn num_initial_quotients() -> usize {
            #num_init_constraints
        }

        fn num_consistency_quotients() -> usize {
            #num_cons_constraints
        }

        fn num_transition_quotients() -> usize {
            #num_tran_constraints
        }

        fn num_terminal_quotients() -> usize {
            #num_term_constraints
        }

        #[allow(unused_variables)]
        fn initial_quotient_degree_bounds(
            interpolant_degree: Degree,
        ) -> Vec<Degree> {
            let zerofier_degree = 1;
            [#init_constraint_degrees].to_vec()
        }

        #[allow(unused_variables)]
        fn consistency_quotient_degree_bounds(
            interpolant_degree: Degree,
            padded_height: usize,
        ) -> Vec<Degree> {
            let zerofier_degree = padded_height as Degree;
            [#cons_constraint_degrees].to_vec()
        }

        #[allow(unused_variables)]
        fn transition_quotient_degree_bounds(
            interpolant_degree: Degree,
            padded_height: usize,
        ) -> Vec<Degree> {
            let zerofier_degree = padded_height as Degree - 1;
            [#tran_constraint_degrees].to_vec()
        }

        #[allow(unused_variables)]
        fn terminal_quotient_degree_bounds(
            interpolant_degree: Degree,
        ) -> Vec<Degree> {
            let zerofier_degree = 1;
            [#term_constraint_degrees].to_vec()
        }
    }
    )
}

/// Given a slice of constraint circuits, return a tuple of [`TokenStream`]s corresponding to code
/// evaluating these constraints as well as their degrees. In particular:
/// 1. The first stream contains code that, when evaluated, produces the constraints' degrees,
/// 1. the second stream contains code that, when evaluated, produces the constraints' values, with
///     the input type for the base row being `BFieldElement`, and
/// 1. the third stream is like the second, except that the input type for the base row is
///    `XFieldElement`.
fn tokenize_circuits<II: InputIndicator>(
    constraint_circuits: &mut [ConstraintCircuit<II>],
) -> (TokenStream, TokenStream, TokenStream) {
    if constraint_circuits.is_empty() {
        return (quote!(), quote!(vec![]), quote!(vec![]));
    }

    // Sanity check: all node IDs must be unique.
    // This also counts the number of times each node is referenced.
    ConstraintCircuit::assert_has_unique_ids(constraint_circuits);

    // Get all unique reference counts.
    let mut visited_counters = constraint_circuits
        .iter()
        .flat_map(|constraint| constraint.get_all_visited_counters())
        .collect_vec();
    visited_counters.sort_unstable();
    visited_counters.dedup();

    // Declare all shared variables, i.e., those with a visit count greater than 1.
    // These declarations must be made starting from the highest visit count.
    // Otherwise, the resulting code will refer to bindings that have not yet been made.
    let shared_declarations = visited_counters
        .into_iter()
        .filter(|&x| x > 1)
        .rev()
        .map(|visit_count| declare_nodes_with_visit_count(visit_count, constraint_circuits))
        .collect_vec();

    let (base_constraints, ext_constraints): (Vec<_>, Vec<_>) = constraint_circuits
        .iter()
        .partition(|constraint| constraint.evaluates_to_base_element());

    // The order of the constraints' degrees must match the order of the constraints.
    // Hence, listing the degrees is only possible after the partition into base and extension
    // constraints is known.
    let tokenized_degree_bounds = base_constraints
        .iter()
        .chain(ext_constraints.iter())
        .map(|circuit| match circuit.degree() {
            d if d > 1 => quote!(interpolant_degree * #d - zerofier_degree),
            d if d == 1 => quote!(interpolant_degree - zerofier_degree),
            _ => unreachable!("Constraint degree must be positive"),
        })
        .collect_vec();
    let tokenized_degree_bounds = quote!(#(#tokenized_degree_bounds),*);

    let tokenize_constraint_evaluation = |constraints: &[&ConstraintCircuit<II>]| {
        constraints
            .iter()
            .map(|constraint| evaluate_single_node(1, constraint, &HashSet::default()))
            .collect_vec()
    };
    let tokenized_base_constraints = tokenize_constraint_evaluation(&base_constraints);
    let tokenized_ext_constraints = tokenize_constraint_evaluation(&ext_constraints);

    // If there are no base constraints, the type needs to be explicitly declared.
    let tokenized_bfe_base_constraints = match base_constraints.is_empty() {
        true => quote!(let base_constraints: [BFieldElement; 0] = []),
        false => quote!(let base_constraints = [#(#tokenized_base_constraints),*]),
    };
    let tokenized_bfe_constraints = quote!(
        #(#shared_declarations)*
        #tokenized_bfe_base_constraints;
        let ext_constraints = [#(#tokenized_ext_constraints),*];
        base_constraints
            .into_iter()
            .map(|bfe| bfe.lift())
            .chain(ext_constraints.into_iter())
            .collect()
    );

    let tokenized_xfe_constraints = quote!(
        #(#shared_declarations)*
        let base_constraints = [#(#tokenized_base_constraints),*];
        let ext_constraints = [#(#tokenized_ext_constraints),*];
        base_constraints
            .into_iter()
            .chain(ext_constraints.into_iter())
            .collect()
    );

    (
        tokenized_degree_bounds,
        tokenized_bfe_constraints,
        tokenized_xfe_constraints,
    )
}

/// Produce the code to evaluate code for all nodes that share a value number of
/// times visited. A value for all nodes with a higher count than the provided are assumed
/// to be in scope.
fn declare_nodes_with_visit_count<II: InputIndicator>(
    requested_visited_count: usize,
    circuits: &[ConstraintCircuit<II>],
) -> TokenStream {
    let mut scope: HashSet<usize> = HashSet::new();

    let tokenized_circuits = circuits
        .iter()
        .filter_map(|circuit| {
            declare_single_node_with_visit_count(circuit, requested_visited_count, &mut scope)
        })
        .collect_vec();
    quote!(#(#tokenized_circuits)*)
}

fn declare_single_node_with_visit_count<II: InputIndicator>(
    circuit: &ConstraintCircuit<II>,
    requested_visited_count: usize,
    scope: &mut HashSet<usize>,
) -> Option<TokenStream> {
    // Don't declare a node twice.
    if scope.contains(&circuit.id) {
        return None;
    }

    // A higher-than-requested visit counter means the node is already in global scope, albeit not
    // necessarily in the passed-in scope.
    if circuit.visited_counter > requested_visited_count {
        return None;
    }

    let CircuitExpression::BinaryOperation(_, lhs, rhs) = &circuit.expression else {
        // Constants are already (or can be) trivially declared.
        return None;
    };

    // If the visited counter is not yet exact, start recursing on the BinaryOperation's children.
    if circuit.visited_counter < requested_visited_count {
        let out_left = declare_single_node_with_visit_count(
            &lhs.as_ref().borrow(),
            requested_visited_count,
            scope,
        );
        let out_right = declare_single_node_with_visit_count(
            &rhs.as_ref().borrow(),
            requested_visited_count,
            scope,
        );
        return match (out_left, out_right) {
            (None, None) => None,
            (Some(l), None) => Some(l),
            (None, Some(r)) => Some(r),
            (Some(l), Some(r)) => Some(quote!(#l #r)),
        };
    }

    // Declare a new binding.
    assert_eq!(circuit.visited_counter, requested_visited_count);
    let binding_name = get_binding_name(circuit);
    let evaluation = evaluate_single_node(requested_visited_count, circuit, scope);

    let is_new_insertion = scope.insert(circuit.id);
    assert!(is_new_insertion);

    Some(quote!(let #binding_name = #evaluation;))
}

/// Return a variable name for the node. Returns `point[n]` if node is just
/// a value from the codewords. Otherwise returns the ID of the circuit.
fn get_binding_name<II: InputIndicator>(circuit: &ConstraintCircuit<II>) -> TokenStream {
    match &circuit.expression {
        CircuitExpression::BConstant(bfe) => tokenize_bfe(bfe),
        CircuitExpression::XConstant(xfe) => tokenize_xfe(xfe),
        CircuitExpression::Input(idx) => quote!(#idx),
        CircuitExpression::Challenge(challenge) => {
            let challenge_ident = format_ident!("{challenge}");
            quote!(challenges.get_challenge(#challenge_ident))
        }
        CircuitExpression::BinaryOperation(_, _, _) => {
            let node_ident = format_ident!("node_{}", circuit.id);
            quote!(#node_ident)
        }
    }
}

fn tokenize_bfe(bfe: &BFieldElement) -> TokenStream {
    let raw_u64 = bfe.raw_u64();
    quote!(BFieldElement::from_raw_u64(#raw_u64))
}

fn tokenize_xfe(xfe: &XFieldElement) -> TokenStream {
    let coeff_0 = tokenize_bfe(&xfe.coefficients[0]);
    let coeff_1 = tokenize_bfe(&xfe.coefficients[1]);
    let coeff_2 = tokenize_bfe(&xfe.coefficients[2]);
    quote!(XFieldElement::new([#coeff_0, #coeff_1, #coeff_2]))
}

/// Recursively construct the code for evaluating a single node.
fn evaluate_single_node<II: InputIndicator>(
    requested_visited_count: usize,
    circuit: &ConstraintCircuit<II>,
    scope: &HashSet<usize>,
) -> TokenStream {
    let binding_name = get_binding_name(circuit);

    // Don't declare a node twice.
    if scope.contains(&circuit.id) {
        return binding_name;
    }

    // The binding must already be known.
    if circuit.visited_counter > requested_visited_count {
        return binding_name;
    }

    // Constants have trivial bindings.
    let CircuitExpression::BinaryOperation(binop, lhs, rhs) = &circuit.expression else {
        return binding_name;
    };

    let lhs = lhs.as_ref().borrow();
    let rhs = rhs.as_ref().borrow();
    let evaluated_lhs = evaluate_single_node(requested_visited_count, &lhs, scope);
    let evaluated_rhs = evaluate_single_node(requested_visited_count, &rhs, scope);
    quote!((#evaluated_lhs) #binop (#evaluated_rhs))
}

/// Given a substitution rule, i.e., a `ConstraintCircuit` of the form `x - expr`, generate code
/// that evaluates `expr`.
fn substitution_rule_to_code<II: InputIndicator>(circuit: ConstraintCircuit<II>) -> TokenStream {
    let BinaryOperation(BinOp::Sub, new_var, expr) = circuit.expression else {
        panic!("Substitution rule must be a subtraction.");
    };
    let Input(_) = new_var.as_ref().borrow().expression else {
        panic!("Substitution rule must be a simple substitution.");
    };

    let expr = expr.as_ref().borrow().to_owned();
    evaluate_single_node(usize::MAX, &expr, &HashSet::new())
}

/// Given all substitution rules, generate the code that evaluates them in order.
/// This includes generating the columns that are to be filled using the substitution rules.
#[allow(clippy::too_many_arguments)]
fn generate_degree_lowering_table_code(
    init_base_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    cons_base_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    tran_base_substitutions: &[ConstraintCircuitMonad<DualRowIndicator>],
    term_base_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    init_ext_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    cons_ext_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    tran_ext_substitutions: &[ConstraintCircuitMonad<DualRowIndicator>],
    term_ext_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
) -> TokenStream {
    let num_new_base_cols = init_base_substitutions.len()
        + cons_base_substitutions.len()
        + tran_base_substitutions.len()
        + term_base_substitutions.len();
    let num_new_ext_cols = init_ext_substitutions.len()
        + cons_ext_substitutions.len()
        + tran_ext_substitutions.len()
        + term_ext_substitutions.len();

    // A zero-variant enum cannot be annotated with `repr(usize)`.
    let base_repr_usize = match num_new_base_cols == 0 {
        true => quote!(),
        false => quote!(#[repr(usize)]),
    };
    let ext_repr_usize = match num_new_ext_cols == 0 {
        true => quote!(),
        false => quote!(#[repr(usize)]),
    };
    let use_challenge_ids = match num_new_ext_cols == 0 {
        true => quote!(),
        false => quote!(
            use crate::table::challenges::ChallengeId::*;
        ),
    };

    let base_columns = (0..num_new_base_cols)
        .map(|i| format_ident!("DegreeLoweringBaseCol{i}"))
        .map(|ident| quote!(#ident))
        .collect_vec();
    let ext_columns = (0..num_new_ext_cols)
        .map(|i| format_ident!("DegreeLoweringExtCol{i}"))
        .map(|ident| quote!(#ident))
        .collect_vec();

    let fill_base_columns_code = generate_fill_base_columns_code(
        init_base_substitutions,
        cons_base_substitutions,
        tran_base_substitutions,
        term_base_substitutions,
    );
    let fill_ext_columns_code = generate_fill_ext_columns_code(
        init_ext_substitutions,
        cons_ext_substitutions,
        tran_ext_substitutions,
        term_ext_substitutions,
    );

    quote!(
        use ndarray::s;
        use ndarray::ArrayView2;
        use ndarray::ArrayViewMut2;
        use strum::EnumCount;
        use strum_macros::Display;
        use strum_macros::EnumCount as EnumCountMacro;
        use strum_macros::EnumIter;
        use twenty_first::shared_math::b_field_element::BFieldElement;
        use twenty_first::shared_math::x_field_element::XFieldElement;

        #use_challenge_ids
        use crate::table::challenges::Challenges;
        use crate::table::master_table::NUM_BASE_COLUMNS;
        use crate::table::master_table::NUM_EXT_COLUMNS;

        pub const BASE_WIDTH: usize = DegreeLoweringBaseTableColumn::COUNT;
        pub const EXT_WIDTH: usize = DegreeLoweringExtTableColumn::COUNT;
        pub const FULL_WIDTH: usize = BASE_WIDTH + EXT_WIDTH;

        // This file has been auto-generated. Any modifications _will_ be lost.
        // To re-generate, execute:
        // `cargo run --bin constraint-evaluation-generator`

        #base_repr_usize
        #[derive(Display, Debug, Clone, Copy, PartialEq, Eq, EnumIter, EnumCountMacro, Hash)]
        pub enum DegreeLoweringBaseTableColumn {
            #(#base_columns),*
        }

        #ext_repr_usize
        #[derive(Display, Debug, Clone, Copy, PartialEq, Eq, EnumIter, EnumCountMacro, Hash)]
        pub enum DegreeLoweringExtTableColumn {
            #(#ext_columns),*
        }

        #[derive(Debug, Clone)]
        pub struct DegreeLoweringTable {}

        impl DegreeLoweringTable {
            #fill_base_columns_code
            #fill_ext_columns_code
        }
    )
}

fn generate_fill_base_columns_code(
    init_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    cons_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    tran_substitutions: &[ConstraintCircuitMonad<DualRowIndicator>],
    term_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
) -> TokenStream {
    let deterministic_section_start =
        master_table::NUM_BASE_COLUMNS - degree_lowering_table::BASE_WIDTH;

    let num_init_substitutions = init_substitutions.len();
    let num_cons_substitutions = cons_substitutions.len();
    let num_tran_substitutions = tran_substitutions.len();
    let num_term_substitutions = term_substitutions.len();

    let init_col_indices = (0..num_init_substitutions)
        .map(|i| i + deterministic_section_start)
        .collect_vec();
    let cons_col_indices = (0..num_cons_substitutions)
        .map(|i| i + deterministic_section_start + num_init_substitutions)
        .collect_vec();
    let tran_col_indices = (0..num_tran_substitutions)
        .map(|i| i + deterministic_section_start + num_init_substitutions + num_cons_substitutions)
        .collect_vec();
    let term_col_indices = (0..num_term_substitutions)
        .map(|i| {
            i + deterministic_section_start
                + num_init_substitutions
                + num_cons_substitutions
                + num_tran_substitutions
        })
        .collect_vec();

    let init_substitutions = init_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();
    let cons_substitutions = cons_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();
    let tran_substitutions = tran_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();
    let term_substitutions = term_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();

    let single_row_substitutions = |indices: Vec<usize>, substitutions: Vec<TokenStream>| {
        assert_eq!(indices.len(), substitutions.len());
        if indices.is_empty() {
            return quote!();
        }
        quote!(
            master_base_table.rows_mut().into_iter().for_each(|mut row| {
                #(
                let (base_row, mut det_col) =
                    row.multi_slice_mut((s![..#indices],s![#indices..#indices + 1]));
                det_col[0] = #substitutions;
                )*
            });
        )
    };
    let dual_row_substitutions = |indices: Vec<usize>, substitutions: Vec<TokenStream>| {
        assert_eq!(indices.len(), substitutions.len());
        if indices.is_empty() {
            return quote!();
        }
        quote!(
            for curr_row_idx in 0..master_base_table.nrows() - 1 {
                let next_row_idx = curr_row_idx + 1;
                let (mut curr_base_row, next_base_row) = master_base_table.multi_slice_mut((
                    s![curr_row_idx..curr_row_idx + 1, ..],
                    s![next_row_idx..next_row_idx + 1, ..],
                ));
                let mut curr_base_row = curr_base_row.row_mut(0);
                let next_base_row = next_base_row.row(0);
                #(
                let (current_base_row, mut det_col) =
                    curr_base_row.multi_slice_mut((s![..#indices], s![#indices..#indices + 1]));
                det_col[0] = #substitutions;
                )*
            }
        )
    };

    let init_substitutions = single_row_substitutions(init_col_indices, init_substitutions);
    let cons_substitutions = single_row_substitutions(cons_col_indices, cons_substitutions);
    let tran_substitutions = dual_row_substitutions(tran_col_indices, tran_substitutions);
    let term_substitutions = single_row_substitutions(term_col_indices, term_substitutions);

    quote!(
    #[allow(unused_variables)]
    pub fn fill_deterministic_base_columns(master_base_table: &mut ArrayViewMut2<BFieldElement>) {
        assert_eq!(NUM_BASE_COLUMNS, master_base_table.ncols());
        #init_substitutions
        #cons_substitutions
        #tran_substitutions
        #term_substitutions
    }
    )
}

fn generate_fill_ext_columns_code(
    init_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    cons_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
    tran_substitutions: &[ConstraintCircuitMonad<DualRowIndicator>],
    term_substitutions: &[ConstraintCircuitMonad<SingleRowIndicator>],
) -> TokenStream {
    let deterministic_section_start =
        master_table::NUM_EXT_COLUMNS - degree_lowering_table::EXT_WIDTH;

    let num_init_substitutions = init_substitutions.len();
    let num_cons_substitutions = cons_substitutions.len();
    let num_tran_substitutions = tran_substitutions.len();
    let num_term_substitutions = term_substitutions.len();

    let init_col_indices = (0..num_init_substitutions)
        .map(|i| i + deterministic_section_start)
        .collect_vec();
    let cons_col_indices = (0..num_cons_substitutions)
        .map(|i| i + deterministic_section_start + num_init_substitutions)
        .collect_vec();
    let tran_col_indices = (0..num_tran_substitutions)
        .map(|i| i + deterministic_section_start + num_init_substitutions + num_cons_substitutions)
        .collect_vec();
    let term_col_indices = (0..num_term_substitutions)
        .map(|i| {
            i + deterministic_section_start
                + num_init_substitutions
                + num_cons_substitutions
                + num_tran_substitutions
        })
        .collect_vec();

    let init_substitutions = init_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();
    let cons_substitutions = cons_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();
    let tran_substitutions = tran_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();
    let term_substitutions = term_substitutions
        .iter()
        .map(|c| substitution_rule_to_code(c.circuit.as_ref().borrow().to_owned()))
        .collect_vec();

    let single_row_substitutions = |indices: Vec<usize>, substitutions: Vec<TokenStream>| {
        assert_eq!(indices.len(), substitutions.len());
        if indices.is_empty() {
            return quote!();
        }
        quote!(
            for row_idx in 0..master_base_table.nrows() - 1 {
                let base_row = master_base_table.row(row_idx);
                let mut extension_row = master_ext_table.row_mut(row_idx);
                #(
                    let (ext_row, mut det_col) =
                        extension_row.multi_slice_mut((s![..#indices],s![#indices..#indices + 1]));
                    det_col[0] = #substitutions;
                )*
            }
        )
    };
    let dual_row_substitutions = |indices: Vec<usize>, substitutions: Vec<TokenStream>| {
        assert_eq!(indices.len(), substitutions.len());
        if indices.is_empty() {
            return quote!();
        }
        quote!(
            for curr_row_idx in 0..master_base_table.nrows() - 1 {
                let next_row_idx = curr_row_idx + 1;
                let current_base_row = master_base_table.row(curr_row_idx);
                let next_base_row = master_base_table.row(next_row_idx);
                let (mut curr_ext_row, next_ext_row) = master_ext_table.multi_slice_mut((
                    s![curr_row_idx..curr_row_idx + 1, ..],
                    s![next_row_idx..next_row_idx + 1, ..],
                ));
                let mut curr_ext_row = curr_ext_row.row_mut(0);
                let next_ext_row = next_ext_row.row(0);
                #(
                let (current_ext_row, mut det_col) =
                    curr_ext_row.multi_slice_mut((s![..#indices], s![#indices..#indices + 1]));
                det_col[0] = #substitutions;
                )*
            }
        )
    };

    let init_substitutions = single_row_substitutions(init_col_indices, init_substitutions);
    let cons_substitutions = single_row_substitutions(cons_col_indices, cons_substitutions);
    let tran_substitutions = dual_row_substitutions(tran_col_indices, tran_substitutions);
    let term_substitutions = single_row_substitutions(term_col_indices, term_substitutions);

    quote!(
        #[allow(unused_variables)]
        pub fn fill_deterministic_ext_columns(
            master_base_table: ArrayView2<BFieldElement>,
            master_ext_table: &mut ArrayViewMut2<XFieldElement>,
            challenges: &Challenges,
        ) {
            assert_eq!(NUM_BASE_COLUMNS, master_base_table.ncols());
            assert_eq!(NUM_EXT_COLUMNS, master_ext_table.ncols());
            assert_eq!(master_base_table.nrows(), master_ext_table.nrows());
            #init_substitutions
            #cons_substitutions
            #tran_substitutions
            #term_substitutions
        }
    )
}
