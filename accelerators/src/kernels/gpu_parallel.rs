use gpu_accelerator::FutharkContext;


/**
 * GpuParallel is a struct with methods for interacting with GPU kernels 
 * written in Futhark. 
 * 
 * The workflow for using GpuParallel from in rust starting from the raw 
 * futhark code is as follows:
 * 
 * First, a .fut file is written tha does the needed computation. There should
 * be and 'entry' point in the .fut file that specifies any i/o needed from/to 
 * rust.
 * 
 * Next, use the genfut (a compiler that generates rust bindings for the futhark)
 * is used to generator a rust library that will interact with futhark code.
 * 
 * After this, the generated library (gpu_accelerator in this case) can be 
 * called from rust.
 */

#[derive(Debug)]
pub struct GpuParallel;

impl GpuParallel {
    #[allow(dead_code)]
    pub fn test_gpu_kernels(number: u64) -> u64 {
        let mut ctx = FutharkContext::new().unwrap();
        ctx.test_gpu_kernel(number).unwrap()
    }
}

#[cfg(test)]
pub(crate) mod gpu_kernel_tests {
    use super::GpuParallel;

    #[test]
    pub fn gpu_kernel_works() {
        let number = 10;
        let number_plus_1 = GpuParallel::test_gpu_kernels(number);
        assert!(number_plus_1 == number + 1);
    }
}
