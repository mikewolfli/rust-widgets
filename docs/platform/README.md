<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# Platform documentation (`docs/platform/`)

Cross-OS platform documentation that does not belong to a single backend's source
tree. The per-backend implementation notes still live next to the code
(`src/platform/<os>/status.md`); this directory holds the **cross-cutting** view.

| 文档 | 内容 |
|---|---|
| [`real-device-verification.md`](real-device-verification.md) | **所有操作系统相关真机 / 真实运行时验证**的统一台账：每条都标出「已确认 / 仅编译 / 已过期 / 记录冲突 / 未验证」，并附日期、硬件与原始证据 |
| [`../plans/platform_capability_matrix.md`](../plans/platform_capability_matrix.md) | 自动生成的控件能力矩阵（后端 × 控件），由 `tools/generate_platform_capability_matrix.py` 产出 |
| [`../plans/platform_differences.md`](../plans/platform_differences.md) | 平台差异的语义说明 |
| `src/platform/macos/status.md` | macOS（`cocoa` + `macos-objc2-preview`）自身状态 |
| `src/platform/ios/status.md` | iOS 自身状态 |
| `src/platform/android/status.md` | Android 自身状态 |
| `src/platform/harmony/status.md` | HarmonyOS（OpenHarmony）自身状态 |

## 记账约定（写在这里，避免漂移）

任何「某平台可用 / 已验证」的结论，按它在哪个宿主上被证实来归类，**不得混用**：

1. **真机 / 原生运行时**：必须给出**命令 + 原始输出**，并写上宿主（OS 版本、CPU、工具链）与日期。
2. **仅编译 / 链接**：必须显式写出 `compile-only` / `build+link only`，**不得**读作"能跑"。
3. **模拟器 != 真机**：iOS Simulator、Android Emulator 上跑过，写「模拟器」，真机另记；HarmonyOS 的 `previewer` 不算真机。
4. **过期结论**：在更早修订上跑过、当前 `HEAD` 未复跑的，标「已过期」，并写出**复跑所需的宿主**。
5. **冲突**：仓库内出现互相矛盾的记录时，列出来源，标「记录冲突」，不要单方面采信。
