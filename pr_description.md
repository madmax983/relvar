💡 The Spark
Signal processing relies on the Fourier Transform to convert time-domain signals into the frequency domain. What if we could perform this directly inside our relational engine? By treating time series data as a relation, we can declaratively compute the Discrete Fourier Transform (DFT).

🚀 The Feature
Implemented the `RelationalFourier` module in `relvar/src/experimental/fourier.rs`.
The DFT is computed by dynamically constructing a basis of frequencies (using a cartesian product, `Extend`, and mathematical functions) and then applying a `Summarize` aggregation to sum the real and imaginary components over time.

🔭 The Potential
This enables direct spectral analysis of time-series data without exporting it from the database. It proves that `relvar` can perform advanced numerical operations natively via relational extensions.

⚠️ Risk
Low. This is fully isolated within the `relvar/src/experimental/fourier.rs` module and does not impact any core system logic.