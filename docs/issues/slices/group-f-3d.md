| R451 | 3D 主线 | **当前是纯 2D 光栅化栈，不具备 3D 变换能力**：wgpu 两个管线 `depth_stencil: None`（renderer.rs:391-395, 447-451）、render pass 无 depth attachment（:660）、顶点着色器直接输出 clip-space 不做变换（shaders.rs:55-63）；WgpuDrawCommand 全集 2D（commands.rs:42-473）坐标一律物理像素左上原点；CPU 路径与 wgpu 路径同一命令集均无 Z。 | 928 | ⬜ |
| R452 | 3D 主线 | **projection.rs 与 3D 无关**（是投影仪/演示模式控制器）→ 更名 presentation.rs 或新模块 math3d.rs，避免 API 检索歧义。 | 928 | ⬜ |
| R453 | 3D 主线 | 批量路径每矩形一次 draw+动态偏移 bind group 无 instancing（:668-674）——3D 化前先换 instanced attributes 有独立收益。 | 928 | ⬜ |
| R454 | 3D 主线 | 已有伪 3D 资产：Bevel Raised/Inset 边缘明暗（bevel.rs:93-129）、渐变、BoxShadow 真模糊、SceneLayer.z_index 平铺层（scene.rs:55-108）、批回放 TransformState **仅 dx/dy/opacity**（batch.rs:404-447）。 | 928 | ⬜ |
| R455 | 3D 主线 | 缺口按层：① 矩阵链（Mat4/Mat3+Transform 栈，命令支持矩形四角经矩阵变换为四边形+透视分割三角化）；② Shader（VS 加 uMVP；FS 逐像素 SDF 在变换下反走样）；③ 深度（控件级 3D 可先用画家算法按 z_index 排序替代）；④ 层合成（per-widget offscreen 层 save-layer/restore+效果管线——backdrop blur、景深、翻转封面共同前置）；⑤ 光照（立体按钮可用 2D 近似 bevel+gradient 已够；卡片透视需法线+简单方向光）；⑥ 文本（透视下先光栅到层再变换层——依赖④）。 | 928 | ⬜ |
| R456 | 3D 主线 | **补齐路径（优先级序，对照 参考工具包）**：P0 仿射层（对应 graphics view item transform）——Transform 栈进 TransformState 与软件光栅器（路径/图像双线性重采样）+wgpu VS 矩阵，覆盖卡片倾斜/按钮按压立体化/可旋转控件；P1 离屏层与效果（对应 declarative UI layers+ShaderEffectSource）——`RenderCommand::PushLayer/PopLayer`，效果首推 backdrop blur（**同时补齐 macOS vibrancy 缺口，一份基建两处收益**）；P2 伪 3D（对应 declarative UI Rotation 伪 3D）——Mat4+透视分割提供 `set_rotation(axis,angle,origin)` Item 级 API；P3 真 3D（对应 quick 3 d）——depth buffer/mesh/相机仅当需求成立再做，retained-mode GUI 库性价比低于"层+透视"组合，建议长期缓行。`prefers_reduced_motion()` 应同时关停 3D 动效。 | 928 | ⬜ |
| R457 | 3D 主线 | **门控决策（用户确认 2026-10-23）**：上述 P0–P3 全部 3D 能力置于 Cargo feature 门控之后，**默认不编译**： | 928 | ⬜ |
| R458 | 3D 主线 | `Cargo.toml` 增加 feature（建议名 `3d`，备选 `math3d`），不进 `default` 集； | 928 | ⬜ |
| R459 | 3D 主线 | 新模块（`math3d.rs`、PushLayer/PopLayer、透视 API）整体 `#[cfg(feature = "3d")]`；对现有代码的侵入点（batch.rs TransformState 扩展、wgpu VS 矩阵路径）用 cfg 隔离； | 928 | ⬜ |
| R460 | 3D 主线 | 门控关闭时现有 2D 路径零行为变化、二进制零增量； | 928 | ⬜ |
| R461 | 3D 主线 | CI 双矩阵：`--no-default-features` 与 `--all-features` 均须编译通过，防门控腐烂。 | 928 | ⬜ |
