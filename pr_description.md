# 🌟 Nova: Relational Discrete Fourier Transform (DFT)

💡 **The Spark:** I realized that we have time-series analysis tools, but no way to extract frequencies from signals using purely relational data!

🚀 **The Feature:** Implemented `RelationalDFT` to calculate a Discrete Fourier Transform purely in relational algebra. Using relational `join`, `extend` (with `cos`/`sin`), and `summarize`, a time-domain relation is gracefully transformed into a frequency-domain spectrum.

🔮 **The Potential:** Can be used to process real-time streams recursively, identify frequencies in IoT data, compress relational signals, or build a purely relational audio equalizer.

⚠️ **Risk:** Low. Fully isolated in `src/experimental/fourier.rs`.
