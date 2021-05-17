#[macro_export]
macro_rules! impl_empty_runtime {
    ($ty: ty) => {
        impl ya_runtime_sdk::Runtime for $ty {
            fn deploy<'a>(
                &mut self,
                _: &mut ya_runtime_sdk::Context<Self>,
            ) -> ya_runtime_sdk::OutputResponse<'a> {
                unimplemented!()
            }

            fn start<'a>(
                &mut self,
                _: &mut ya_runtime_sdk::Context<Self>,
            ) -> ya_runtime_sdk::OutputResponse<'a> {
                unimplemented!()
            }

            fn stop<'a>(
                &mut self,
                _: &mut ya_runtime_sdk::Context<Self>,
            ) -> ya_runtime_sdk::EmptyResponse<'a> {
                unimplemented!()
            }

            fn run_command<'a>(
                &mut self,
                _: ya_runtime_sdk::RunProcess,
                _: ya_runtime_sdk::RuntimeMode,
                _: &mut ya_runtime_sdk::Context<Self>,
            ) -> ya_runtime_sdk::ProcessIdResponse<'a> {
                unimplemented!()
            }
        }
    };
}
