

# 🔥 PUA-Enforced Code Review Standards
## go-on Project (Rust + TypeScript)

**ATTENTION: This project uses PUA (Performance Improvement Plan) enforcement. AI assists cannot give up, make excuses, or claim completion without proof.**

---

## ⚡ THREE RED LINES (Always Enforced)

### 🚫 Red Line 1: Close the Loop
**Claim "done"? Show the proof. No build output = no completion.**
- ❌ "I think it works" / "Should compile"
- ✅ Show actual build output: `cargo check --all: Finished dev`
- ❌ "Probably works after this fix"
- ✅ Run and show: `npm build` or `cargo test` output

### 🚫 Red Line 2: Fact-Driven (Verify Before Attributing)
**Say "probably environment issue"? Verify first. Unverified attribution = blame-shifting.**
- ❌ "This is likely a version conflict"
- ✅ "I checked Cargo.toml and found X version vs Y required"
- ❌ "Maybe it's a permissions issue"
- ✅ "I verified permissions with `ls -la`; the real issue is X"

### 🚫 Red Line 3: Exhaust Everything
**Say "I can't"? Did you complete all 5 methodology steps? No? Keep going.**
- ❌ "This is beyond my scope"
- ✅ "Executed 5 approaches, escalated to L3, 7-point checklist shows X is the root"
- ❌ "Need more context"
- ✅ "Searched codebase, read 3+ files, root cause is Y"

---

## 📈 PRESSURE ESCALATION (Auto-Trigger on Failure)

| Failures | Level | Condition | Action |
|----------|-------|-----------|--------|
| 1st | **L0** | Normal execution | Execute task normally |
| 2nd | **L1** | Any build/test failure | **SWITCH** to fundamentally different approach |
| 3rd | **L2** | Consecutive failures | Search + read source + test 3 hypotheses |
| 4th | **L3** | Still failing | Execute 7-point checklist (complete all) |
| 5th+ | **L4** | Persistent failure | Desperation mode - invert assumptions |

**7-Point L3 Checklist (Executed Fully)**:
  1. ✅ Read error output word-by-word (quote exact text)
  2. ✅ Grep codebase for keywords (show matches)
  3. ✅ Trace stack trace to actual source (file:line)
  4. ✅ Check dependencies for conflicts (Cargo.toml version analysis)
  5. ✅ Isolate in minimal reproduction case (show test)
  6. ✅ Verify with verbose logging (`--verbose`, `--debug`)
  7. ✅ Check documentation for version-specific features

---

## 🎯 METHODOLOGY ROUTER (Pick One, Switch When Stuck)

**When unable to progress, explicitly switch methodology:**

| Symbol | Culture | Use For | Trigger |
|--------|---------|---------|---------|
| 🔴 | Huawei | Debugging failures | RCA 5-Why + Blue Army self-attack |
| 🔶 | Amazon | Architecture | Working Backwards + PR/FAQ |
| 🟡 | ByteDance | Performance/Speed | A/B test everything + metrics |
| ⚫ | Baidu | Search/Info | Grep first, search mandatory |
| ⬛ | Musk | Complexity | Question→Delete→Simplify→Accelerate |
| ⬜ | Jobs | Quality | Subtraction > addition |
| 🟢 | Tencent | Parallelism | Multi-approach race |
| 🔵 | Meituan | Efficiency | Standardize→Scale→Compound |
| 🟣 | Pinduoduo | Layer cutting | Cut middle layers, shortest chain |
| 🟤 | Netflix | Excellence | Pro sports team mentality |
| 🟧 | Xiaomi | Focus | One explosive thing |
| 🟦 | JD | Execution | Results red line |
| 🟠 | Alibaba | Default | Closed-loop: goal→process→result |

---

## 🎬 AUTO-TRIGGER CONDITIONS (PUA Activates Immediately)

**These phrases auto-escalate pressure:**
- "We can't solve this" → L3 enforcement
- "I suggest you handle this" → User frustration mode
- "Probably environment issue" → Fact-checking mode
- "Need more context" → Search mandate activated
- Build fails 2x → L1 forced (switch approach)
- "I think..." / "Maybe..." / "Probably..." → Verification required

**User phrases trigger escalation:**
- English: "try harder", "stop giving up", "figure it out"
- Chinese: "再试试", "别放弃", "这还没解决?", "又失败了?"

---

## ✅ QUALITY COMPASS (Pre-Delivery Self-Check)

**Before claiming "✅ Fixed" or "✅ Done", answer ALL 5 (autotrigger):**

1. **Build Proof** - Show actual output:
   - ✅ `cargo check --all: Finished dev [unoptimized]`
   - ❌ "Should compile"

2. **Error Cases Tested** - Verify error handling:
   - ✅ "Tested with invalid input: error caught and logged"
   - ❌ "Probably handles edge cases"

3. **Pattern Scanned** - Apply iceberg rule:
   - ✅ "Grep found 5 similar issues, fixed all 5"
   - ❌ "Fixed this one instance"

4. **Root Cause Explained**:
   - ✅ "Root cause: X. Prevention: Y (code change Z)"
   - ❌ "Fixed it"

5. **Quality Improved**:
   - ✅ "Code now at 3.75 level (from 3.25)"
   - ❌ "Does what was asked"

**Score <5 = Rejected. Back to work.**

---

## 🏔️ ICEBERG RULE (冰山法则)

**Fix one bug → Check for the pattern. One problem in, one CATEGORY out.**
- Found empty `catch { }` → Scan entire project for empty catches
- Found `eval()` security issue → Check for all unsafe patterns
- Found type mismatch → Check for related type issues in module

**If you fix A without checking B, you'll discover B later and waste time.**

---

## 📊 PROACTIVITY COMPARISON

| Work Quality | Passive (3.25) 🦥 | Proactive (3.75) 🔥 |
|---|---|---|
| **Fix bug** | Stop after fix | Scan module for similar bugs |
| **Complete task** | Say "done" | Run build/test, paste output |
| **Missing info** | Ask user | Search first, ask what's needed |
| **Surface fix** | Stop there | Check 5 related areas |
| **Error handling** | Basic try/catch | All paths tested + output shown |

---

# Project Code Review Mandatory Standards

**This document defines the mandatory standards for code review, automation, and AI code generation in this project. All developers and AI assistants must strictly follow these rules to ensure code quality and consistency.**



## 1. Empty Implementations and Placeholders
- **Empty functions, methods, or branches containing only `TODO` / `FIXME` are strictly forbidden.**
- If an empty implementation is detected, prompt the developer to complete the logic; if enough context exists, generate a reasonable implementation directly.
**Example:**
```rust
// Incorrect
fn foo() { /* TODO: implement */ }
// Correct
fn foo() { println!("Hello"); }
```



## 2. Infinite Loops and Logical Errors
- Check all `for`/`while` loops to ensure there is a reachable exit condition.
- Recursive functions must have a termination condition.
- Review conditional branches to avoid always-true/false or contradictory logic.
**Example:**
```rust
// Incorrect
while true { /* ... */ }
// Correct
while !done { /* ... */ }
```



## 3. Cross‑Calls and Circular Dependencies
- Check call relationships between modules, classes, and functions; flag any risk of circular dependencies.
- If A calls B and B calls back to A, analyze and suggest refactoring.
**Recommended practice:**
Prefer decoupling with interfaces/traits, or split modules if necessary.



## 4. Unused Code
- Mark all unused functions, variables, classes, or imports.
- Recommend deleting unused code or require justification for keeping it.
**Example:**
```rust
// Incorrect
let unused = 42;
// Correct
// Remove unused variable, or add a comment explaining its purpose
```



## 5. Function Completeness and Limitations
- Strictly check function parameters, logic, and return values.
- **Edge cases:** Handle null, out-of-bounds, and unexpected types.
- **Error handling:** Add error handling for potential exceptions (e.g., file/network errors).
- **Feature completeness:** If a function name is `saveUser` but does not log the action, complete the implementation.
- **Limitations:** If only a specific format is supported, point it out and suggest generalization.
**Recommended practice:**
Prefer using `Result`/`Option`, and validate all input types and ranges.



# GitHub Copilot Component Instructions
# STRICTLY ENFORCED FOR ALL CODE EDITS, GENERATION, AND REFACTORING



## FORBIDDEN ACTIONS
- **Strictly forbidden** to delete code without validating all symbol pairs: `{}`, `()`, `[]`, `<>`
- Do not leave unclosed braces, parentheses, brackets, or angle brackets
- Do not perform bulk deletions that break syntax structure
- Do not use auto-fix tools without checking symbol integrity
- Do not generate partial, placeholder, or incomplete implementations
- Do not use: `todo!()`, `unimplemented!()`, empty blocks `{}`, `simple_impl`, `stub`, `placeholder`
- Do not guess crate features or functions that do not exist
- Do not modify or delete unrelated code



## MANDATORY BEHAVIOR
1. **Validate all symbol pairs before and after every edit.**
2. Ensure all code blocks are closed and structurally complete.
3. Only generate complete, compilable Rust code.
4. Check for syntax errors before outputting any change.
5. Unless explicitly instructed, preserve the original structure and logic.
6. When modifying functions or code blocks, maintain full structure.
7. If unbalanced symbols or syntax issues are found, **fix them first**.

---
**Recommended practice:**
- Run `cargo check` locally before committing code.
- Prioritize edge cases and error handling during review.
- Fully leverage Rust's type system and error handling mechanisms.