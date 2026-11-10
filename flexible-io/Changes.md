# 0.6.1

- Added `unstable_try_as_dyn` feature and several `try_as_*` methods for `Reader`
  and `Writer` that mirror the functionality of `set_` but are based on compiler
  support of `core::any::try_as_mut` instead of a callee context trait bound.
  These fully eliminate the bound from ever appearing but support is limited to
  certain type and trait combinations as required for specialization soundness.
