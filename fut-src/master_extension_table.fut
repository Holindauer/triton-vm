module ArithmeticDomain = import "arithmetic_domain"
module XFieldElement = import "XFieldElement"
module XfePolynomial = import "xfe_poly"
module BFieldElement = import "BFieldElement"

let NUM_EXT_COLUMNS: i64 = 50

type BFieldElement = BFieldElement.BFieldElement
type ArithmeticDomain = ArithmeticDomain.ArithmeticDomain
type XFieldElement = XFieldElement.XFieldElement
type XfePolynomial [n] = XfePolynomial.XfePolynomial [n]


-- NOTE: MasterExtTable is 

type~ MasterExtTable = {

    num_trace_randomizers: i64,

    trace_domain: ArithmeticDomain,
    randomized_trace_domain : ArithmeticDomain,
    quotient_domain : ArithmeticDomain,
    fri_domain : ArithmeticDomain,

    randmized_trace_table: [][]XFieldElement,
    low_degree_extended_table: [][]XFieldElement, 
    interpolated_polynomials: []XFieldElement
}

-- same for MasterExtTable and MasterBaseTable 
def evaluation_domain (table: MasterExtTable) : ArithmeticDomain =
    if table.quotient_domain.len > table.fri_domain.len
    then table.quotient_domain
    else table.fri_domain

-- low-degree extend all columns of the randomized trace domain table. 
def low_degree_extend_all_collumns (table: MasterExtTable) :  bool = -- : (_, bool) =

    let evaluation_domain: ArithmeticDomain = evaluation_domain table
    let randomized_trace_domain: ArithmeticDomain = table.randomized_trace_domain
    let num_rows: i64 = evaluation_domain.len

    -- get randomized trace table
    let trace_table: [][]XFieldElement = table.randmized_trace_table

    -- func to computes interpolants
    let interpolate_poly = \(trace_column) ->
        ArithmeticDomain.interpolate_xfe_values randomized_trace_domain trace_column

    -- retrieve columns of trace table
    let trace_columns: [][]XFieldElement=
        map 
        (\col_idx -> map (\row_idx -> trace_table[row_idx][col_idx])(iota num_rows)) -- <-- gets column
        (iota NUM_EXT_COLUMNS)

    -- Perform the interpolation for each column
    let interpolation_polynomials =
        map (\trace_column -> interpolate_poly trace_column) trace_columns

    in true




-- TODO: Write a rust crate that directly inputs the data into the entry point
-- NOTE: The intital state setup within this test was collected via the 
-- process described in master_ext_LDE_test_vec_documentation.md
-- == 
-- entry: low_degree_extend_all_columsn_unit_test
-- input { }
-- output { true }
entry low_degree_extend_all_columsn_unit_test 
    (randomized_trace_table_coefficient_values: [][][]u64)
    -- : (low_degree_exteneded_table: [][][]u64) =
    : bool =

    -- setup Master Table state before running LDE 
    let evaluation_domain: ArithmeticDomain = { 
        offset = BFieldElement.new 7u64,
        generator = BFieldElement.new 1532612707718625687u64,
        len = 8192i64
    }
    let randomized_trace_domain: ArithmeticDomain = {
        offset = BFieldElement.new 1u64,
        generator = BFieldElement.new 455906449640507599u64,
        len = 2048i64
    }
    let quotient_domain: ArithmeticDomain = {
        offset = BFieldElement.new 7u64,
        generator = BFieldElement.new 1532612707718625687u64,
        len = 8192i64
    }
    let fri_domain: ArithmeticDomain = {
        offset = BFieldElement.new 7u64,
        generator = BFieldElement.new 1532612707718625687u64,
        len = 8192i64
    }

    -- unpack randomized_trace_table_coefficient_values, convert to XFieldElement
    let randomized_trace_table: [][]XFieldElement =
        map 
        (\row -> map (\x -> {coefficients = (BFieldElement.new x[0], BFieldElement.new x[0], BFieldElement.new x[0])}) row)
        randomized_trace_table_coefficient_values

    in true


