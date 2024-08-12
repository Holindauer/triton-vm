//! The constraint generator is a tool that generates efficient-to-evaluate code
//! for the constraints of Triton Virtual Machine, in particular, for the
//! Arithmetic Intermediate Representation (AIR) constraints of the
//! Zero-Knowledge Proof System underpinning the STARK proof system.
//!
//! The constraints are defined in the Triton VM crate. In order to leverage
//! compiler optimizations, rust code is generated using those constraints.
//!
//! Additionally, the constraints are also translated to Triton Assembly (TASM).
//! This allows Triton VM to evaluate its own constraints, which is essential
//! for recursive proof verification, or Incrementally Verifiable Computation.
//!
//! The constraint generator can be run by executing
//! `cargo run --bin constraint-evaluation-generator`
//! in the root of the repository.

#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

use codegen::common_tasm;
use proc_macro2::TokenStream;
use quote::quote;
use std::fs::write;

use crate::codegen::Codegen;
use crate::codegen::DynamicTasmBackend;
use crate::codegen::RustBackend;
use crate::codegen::StaticTasmBackend;
use crate::constraints::Constraints;

mod codegen;
mod constraints;
mod substitution;

fn main() {
    let mut constraints = Constraints::all();
    let substitutions = constraints.lower_to_target_degree_through_substitutions();
    let degree_lowering_table_code = substitutions.generate_degree_lowering_table_code();

    let constraints = constraints.combine_with_substitution_induced_constraints(substitutions);

    let rust = RustBackend::constraint_evaluation_code(&constraints);

    let tasm_imports = common_tasm::uses();
    let static_tasm = StaticTasmBackend::constraint_evaluation_code(&constraints);
    let dynamic_tasm = DynamicTasmBackend::constraint_evaluation_code(&constraints);
    let tasm = quote! {#tasm_imports #static_tasm #dynamic_tasm};

    write_code_to_file(
        degree_lowering_table_code,
        "triton-vm/src/table/degree_lowering_table.rs",
    );
    write_code_to_file(rust, "triton-vm/src/table/constraints.rs");
    write_code_to_file(tasm, "triton-vm/src/air/tasm_air_constraints.rs");
}

fn write_code_to_file(code: TokenStream, file_name: &str) {
    let syntax_tree = syn::parse2(code).unwrap();
    let code = prettyplease::unparse(&syntax_tree);
    write(file_name, code).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraints_can_be_fetched() {
        let _ = Constraints::test_constraints();
    }

    #[test]
    fn degree_lowering_tables_code_can_be_generated_for_test_constraints() {
        let mut constraints = Constraints::test_constraints();
        let substitutions = constraints.lower_to_target_degree_through_substitutions();
        let _ = substitutions.generate_degree_lowering_table_code();
    }

    #[test]
    fn all_constraints_can_be_fetched() {
        let _ = Constraints::all();
    }

    #[test]
    fn degree_lowering_tables_code_can_be_generated_from_all_constraints() {
        let mut constraints = Constraints::all();
        let substitutions = constraints.lower_to_target_degree_through_substitutions();
        let _ = substitutions.generate_degree_lowering_table_code();
    }

    #[test]
    fn constraints_and_substitutions_can_be_combined() {
        let mut constraints = Constraints::test_constraints();
        let substitutions = constraints.lower_to_target_degree_through_substitutions();
        let _ = constraints.combine_with_substitution_induced_constraints(substitutions);
    }
}
