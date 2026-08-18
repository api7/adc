# ADC Rust 化调查 · Benchmark 结论清单

阶段 0（`adc-differ` Go/No-Go 验证）的全部命题、测试方法与结论。过程细节见 `BENCHMARK.md`，本文件只保留结论。

---

### 1. differ 纯算法性能：Rust vs TS，谁快、快多少？

- **方法**：同一批分层合成数据集（100/1,000/10,000 服务规模 × 无/少量/大量变更），Rust 用 criterion、TS 用 mitata，进程内温启动测量（不含 CLI/Node 冷启动）。
- **结论**：Rust 快约 **2-2.6 倍**（平均 ~2.3x）。两边均随规模线性扩展，无 O(n²) 热点。

### 2. differ 算法还能降低时间复杂度吗？

- **方法**：criterion 数据观察规模扩展趋势。
- **结论**：已经是 **O(N) 线性**，无渐进复杂度可降。剩余空间只有常数因子优化（减少 clone、rayon 并行化）。

### 3. 完整 `adc sync` 流程里，CPU 花在哪个阶段？

- **方法**：`process.cpuUsage()` 逐阶段测量（load_local/lint/init_backend/load_remote/diff/sync），backend 指向本地 mock Admin API 排除真实网关影响。
- **结论**：**`sync` 阶段占 87-91% CPU，`differ` 只占个位数**。differ 从来不是端到端瓶颈。

### 4. 换成聚合提交的 backend（`apisix-standalone`）能否解决 sync 瓶颈？

- **方法**：同一 benchmark 脚本切换 `apisix`（每资源一请求）与 `apisix-standalone`（单请求聚合）两种 backend 实测对比。
- **结论**：能带来 **7-8 倍**端到端提升，但**当前无法落地**——绝大多数生产用户跑的是传统模式（走 etcd），其 Admin API 是逐资源接口、不支持批量提交，这是 API 设计限制，不是"以后能加"。

### 5. sync 阶段是否单核跑满？多核异步运行时能否降低墙钟时间？

- **方法**：a) 把 mock server 隔离到独立进程，排除"server 和被测代码共享线程"的测量污染；b) 分别用 Rust 单线程/多线程 tokio runtime 打同一 mock server；c) 把 mock server 也换成 Rust/tokio 20-worker 多线程实现，重复 b)。
- **结论**：**业务逻辑确实单核跑满**（无 `worker_threads`）；`cpu/wall > 100%` 是 V8 后台 GC 线程导致，不是业务代码在并行。**多核 tokio client 在两种 server（单线程 Node / 20-worker Rust）下均没有优势**，甚至更差——请求粒度太细（每请求 CPU 工作量极小），跨线程协调开销超过收益。多核对 sync 阶段无效，与 differ（单次调用颗粒度粗）的情况相反。

### 6. 把 sync 阶段换成 Rust（reqwest+tokio），开销降多少？

- **方法**：写一个"理论等价"的 Rust 实现（`adc-sync-bench`，reqwest 高层 API + `futures::stream::buffer_unordered`，非裸写），对同一 mock server 发送同样的请求集合，与 TS 的 axios+RxJS 实现对比——**两边都是符合工程实践的正常写法**。
- **结论**：Rust CPU 少约 **13.7 倍**，wall time 少约 **5.6 倍**。这是应采信为决策依据的数字。

### 7. TS 侧 axios+RxJS 的开销，RxJS 和 axios 各占多少？

- **方法**：消融实验，固定同一批请求，只换驱动方式：`rxjs_axios`（真实结构）→ `plain_axios`（去掉 RxJS，手写并发池）→ `plain_fetch`（再把 axios 换成 Node 内置 fetch）。
- **结论**：RxJS 编排开销约占 **22%**；axios 比 fetch/undici **更便宜**（反直觉但两次独立测量方向一致）；去掉 RxJS 之后剩余部分（约 78%）当时被归为"运行时基线成本"——该判断在第 8 点被修正。

### 8. 允许破坏性手段（对象复用、跳过分配、调大堆、暂缓 GC），TS 基线成本能压多低？

- **方法**：脱离 axios 与 RxJS，直接用 `node:http` + 预计算请求体/URL + V8 堆参数调优（`--max-old-space-size` 等），测试是否能进一步降低 CPU。
- **结论**：**能大幅压低**——绕开 axios（不只是 fetch）本身就能省约 53%，调大堆再省约 18%，CPU 差距理论上可从 13.7 倍收窄到约 2.6 倍、wall time 收窄到约 1.25 倍。**但这不是可行方案**：达到这个数字要放弃 axios/RxJS 提供的错误归一化、interceptor、声明式重试等真实能力，属于不可接受的工程倒退，不能视为"TS 优化到位后能追上 Rust"。真正该采信的仍是第 6 点两边都保持工程实践水准时测出的 **13.7 倍 / 5.6 倍**。且这里存在一个对 Rust 有利的不对称性：Rust 不放弃可维护性（正常 reqwest+tokio 写法）就已经接近 TS 放弃可维护性才能摸到的性能。

### 9. `adc-differ` Rust 移植的正确性与开发体验如何？

- **方法**：移植 TS 侧全部 6 个 `libs/differ` 测试文件（46 个用例）到 Rust，逐条对拍。
- **结论**：功能对等，46/46 通过。过程中发现并修复了 TS 原实现两处非平凡的隐藏语义（CREATE/DELETE 事件对嵌套资源 id 的处理不对称）；性能优化阶段还暴露过一次靠测试才能捕获的正确性回归（`resolve_default_type` 误判 stream service）——说明团队若无充分测试覆盖，独立做类似重构有真实风险。

---

## 汇总结论

| 命题 | 结论 |
|---|---|
| differ 语言迁移收益 | 2-2.6 倍，且已是线性算法，无更大空间 |
| sync 阶段语言迁移收益（公平对比） | **13.7 倍 CPU / 5.6 倍 wall，是本次调查里最大的单项收益** |
| 批量提交架构收益 | 7-8 倍，但当前无法落地 |
| 多核并行 | 对 differ 可能有效（未验证），对 sync 阶段无效（已验证） |
| TS 自我优化空间 | 理论上限约 2.6 倍/1.25 倍，但需放弃工程可维护性，不可行 |
| 正确性/移植风险 | 可行，但需要充分的对拍测试兜底，不能想当然 |
