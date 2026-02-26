# MLX Integration Plan - Apple Silicon Optimization

## Why MLX?

MLX is Apple's official ML framework for Apple Silicon (M1/M2/M3/M4), providing:

- **GPU Acceleration:** Native Metal Performance Shaders
- **Unified Memory:** Zero-copy CPU/GPU data transfer
- **2-10x Performance:** Compared to CPU-only frameworks
- **Apple-Optimized:** Built by Apple for MacBooks

## Current Status

**Phase 7 (Feature Engineering)** is implementing ML features but **WITHOUT MLX support**.

Current framework choices:
- `ta` crate (technical indicators, CPU-only)
- `candle` (Hugging Face, cross-platform)
- `linfa` (classical ML, pure Rust)

**MISSING:** MLX for Apple Silicon GPU acceleration

## Required Changes

### 1. Add MLX Dependency

**Cargo.toml:**
```toml
[target.'cfg(all(target_arch = "aarch64", target_os = "macos"))'.dependencies]
mlx-rs = { version = "0.1", optional = true }
metal-rs = "0.1"

[features]
default = []
mlx = ["mlx-rs"]
apple-silicon = ["mlx"]
```

### 2. Create MLX Backend

**File:** `crates/ferrotick-ml/src/mlx_backend.rs`

```rust
#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
mod mlx_backend {
    use mlx_rs::{Device, Tensor};
    
    pub struct MlxFeatureEngine {
        device: Device,
    }
    
    impl MlxFeatureEngine {
        pub fn new() -> Result<Self> {
            let device = Device::gpu()?;  // Apple GPU
            Ok(Self { device })
        }
        
        /// GPU-accelerated RSI
        pub fn compute_rsi(&self, prices: &Tensor) -> Tensor {
            // MLX-optimized RSI on GPU
        }
        
        /// Batch feature computation
        pub fn compute_features_batch(&self, symbols: &[&str]) -> Vec<Tensor> {
            // Parallel GPU computation
        }
    }
}
```

### 3. Update Technical Indicators

**File:** `crates/ferrotick-ml/src/features/technical.rs`

```rust
pub struct TechnicalIndicators {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    mlx_engine: MlxFeatureEngine,  // GPU
    
    #[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
    rsi_cpu: RelativeStrengthIndex,  // CPU fallback
}

impl TechnicalIndicators {
    pub fn compute_rsi(&mut self, prices: &[f64]) -> Vec<f64> {
        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        {
            // GPU path: M1/M2/M3/M4
            let tensor = Tensor::from_slice(prices);
            self.mlx_engine.compute_rsi(&tensor).to_vec()
        }
        
        #[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
        {
            // CPU path: Intel Mac, Linux, Windows
            prices.iter().map(|p| self.rsi_cpu.next(*p)).collect()
        }
    }
}
```

### 4. Add CLI Device Flag

```bash
# Auto-detect (GPU on Mac, CPU elsewhere)
ferrotick ml features AAPL

# Force GPU (Apple Silicon only)
ferrotick ml features AAPL --device gpu

# Force CPU
ferrotick ml features AAPL --device cpu

# Show device info
ferrotick ml device-info
# Output: "Apple M3 Pro GPU (16 cores) - MLX enabled"
```

### 5. Performance Benchmarks

Expected speedups on Apple Silicon:

| Operation | CPU (Intel i9) | M3 Pro GPU | Speedup |
|-----------|----------------|------------|---------|
| RSI (1M bars) | 2.3s | 0.4s | **5.7x** |
| MACD (1M bars) | 3.1s | 0.3s | **10.3x** |
| Feature batch (100 symbols) | 45s | 6s | **7.5x** |
| LSTM inference | 12ms | 1.2ms | **10x** |

### 6. Documentation Updates

**README.md:**
```markdown
## 🚀 Apple Silicon Optimization

Ferrotick is optimized for Apple Silicon with native MLX integration:

- **GPU Acceleration:** Feature computation on Apple GPU
- **Unified Memory:** Zero-copy CPU/GPU transfer
- **2-10x Faster:** Than CPU-only implementations

\`\`\`bash
# Check your device
ferrotick ml device-info
# Output: Apple M3 Pro GPU (16 cores) - MLX enabled
\`\`\`
```

## Implementation Timeline

**After Phase 7 completes:**

1. **Phase 7.1:** Add MLX dependency (1 day)
2. **Phase 7.2:** Create MLX backend (2 days)
3. **Phase 7.3:** GPU-accelerate indicators (2 days)
4. **Phase 7.4:** CLI device management (1 day)
5. **Phase 7.5:** Benchmarks and docs (1 day)

**Total:** ~1 week

## Compatibility Matrix

| Platform | Backend | GPU Support | Performance |
|----------|---------|-------------|-------------|
| **macOS (M1/M2/M3/M4)** | **MLX** | ✅ Apple GPU | ⚡⚡⚡ |
| macOS (Intel) | Candle | ❌ | ⚡⚡ |
| Linux | Candle | ✅ CUDA | ⚡⚡⚡ |
| Windows | Candle | ✅ CUDA | ⚡⚡⚡ |

## Priority

**CRITICAL** - This makes ferrotick the best-performing financial ML library on MacBooks.

Without MLX:
- ❌ Wastes Apple Silicon GPU
- ❌ 5-10x slower on MacBooks
- ❌ Competitive disadvantage

With MLX:
- ✅ Best MacBook performance
- ✅ Native Apple integration
- ✅ 2-10x speedup

## Next Steps

1. Wait for Phase 7 completion
2. Spawn coder agent for MLX integration
3. Update architecture docs
4. Run performance benchmarks
5. Update documentation

---

*Created: Feb 26, 2026*
*Status: PLANNED (pending Phase 7 completion)*
