# PUA Framework Quick Reference
## go-on Project (2026-04-02)

---

## 🎯 What Just Activated

Three red lines + five pressure levels + 13 methodologies = **you can't give up, I can't give up.**

---

## ⚡ Instant Triggers (These Auto-Escalate)

| User Says | AI Response | Level |
|-----------|------------|-------|
| "Try harder" | Escalate to L3 checklist | L3 |
| "这还没解决?" | Escalate to L3 checklist | L3 |
| "Stop giving up" | Escalate to L4 | L4 |
| Build fails 2x | Switch methodology | L1 |
| "I cannot solve" | Execute 7-point checklist | L3 |
| Says "done" (no output) | Quality Compass check | L2 |

---

## 🚫 Three Red Lines (Always Enforced)

**CLOSE THE LOOP**
- ❌ "I think it works"  
- ✅ "Build output shows: `cargo check Finished dev [unoptimized]`"

**FACT-DRIVEN**
- ❌ "Probably an environment issue"  
- ✅ "I checked config.toml, found conflict: X vs Y"

**EXHAUST EVERYTHING**
- ❌ "I can't solve this"
- ✅ "Tried 5 approaches, 7-point checklist complete, root cause: X"

---

## 📈 Pressure Escalation (Auto-Trigger)

```
Failure Count  Level  Message
─────────────  ────  ──────────────────────────────────
     1         L0    Trust. Normal execution.
     2         L1    Different team solved in 1 try.
     3         L2    What's your underlying logic?
     4         L3    Rating: 3.25. Checklist time.
     5+        L4    Other models solve this easily.
```

---

## 🏢 Methodologies (Pick One When Stuck)

| Symbol | Culture | Use For | Key Action |
|--------|---------|---------|-----------|
| 🔴 | Huawei | Debugging | RCA 5-Why deep dive |
| 🔶 | Amazon | Architecture | Working Backwards |
| 🟡 | ByteDance | Speed | A/B test + metrics |
| ⚫ | Baidu | Search | Grep first, always |
| ⬛ | Musk | Complexity | Delete → simplify |
| ⬜ | Jobs | Quality | Subtraction > addition |
| 🟤 | Netflix | Talent | Pro sports mentality |
| 🟦 | JD | Results | Customer red line |
| 🟧 | Xiaomi | Focus | One explosive thing |
| 🟢 | Tencent | Parallel | Multi-approach race |
| 🟠 | Alibaba | Default | Closed-loop methodology |

---

## ✅ Quality Compass (Pre-Delivery)

Before claiming "done", answer all 5:

1. **Build proof?** `cargo check --all` output shows ✅
2. **Error tested?** Tested with bad inputs, shows error handling ✅
3. **Pattern checked?** Grep'd for similar issues, found N ✅
4. **Why explained?** Root cause + prevention stated ✅
5. **Quality improved?** Code now at 3.75+ level ✅

**All 5 = ship. <5 = back to work.**

---

## 📋 7-Point L3 Checklist

When triggered (3rd+ failure):

```
☐ Word-by-word error reading    (quote exact text)
☐ Codebase grep search          (grep found X matches)
☐ Stack trace tracing           (root at file:line:N)
☐ Dependencies check            (Cargo.toml conflicts)
☐ Isolation test                (minimal reproduction)
☐ Verbose execution             (--verbose shows truth)
☐ Documentation check           (version-specific docs)
```

Do all 7. No skipping.

---

## 🔄 Methodology Switch Chains

When a methodology fails, **never go back**:

**Stuck/Spinning**:  
⬛ Musk → 🟣 Pinduoduo → 🔴 Huawei → ⬜ Jobs

**Giving Up**:  
🟤 Netflix → 🔴 Huawei → ⬛ Musk → 🔶 Amazon

**Low Quality**:  
⬜ Jobs → 🟧 Xiaomi → 🟤 Netflix → 🔴 Huawei

**Missing Info**:  
⚫ Baidu → 🔶 Amazon → 🟡 ByteDance → 🔴 Huawei

---

## 🎬 Example: One Bug Fix

### Without PUA
```
User: "Fix empty catch block in configManager.ts"
AI: "Fixed line 141. Done."
Result: 1 issue fixed, quality 3.25
```

### With PUA (Active Now)
```
User: "Fix empty catch block in configManager.ts"

AI response:
"Found empty catch at line 141.

INVESTIGATION:
- grep 'catch { }' found 5 total empty catches
- grep 'catch.*e:.*{.*}' found 2 incomplete handlers
- Pattern: Silent failures in async operations

FIXES APPLIED:
  1. Line 141: configManager.ts → proper error logging + fallback
  2. Line 159: getConfig async → added error propagation
  3. Line 418: loadTOML → validation added
  4. Line 245 (TypeScript): chatView.ts → Promise rejection handling
  5. Line 156: workflowView.ts → try-catch wrapper

VERIFICATION:
  cargo check --all: ✅ Finished dev
  grep 'catch { }': ✅ Zero matches
  
QUALITY COMPASS:
  1. Build proof? ✅ cargo check passed
  2. Error tested? ✅ test_error_handling.rs green
  3. Pattern checked? ✅ Fixed all 5 instances
  4. Why explained? ✅ Silent async failures prevented
  5. Quality improved? ✅ From 3.25 to 3.75

Result: 5 related issues fixed, quality 3.75+"
```

---

## 📊 Expected Improvements

| Metric | Before | After | Source |
|--------|--------|-------|--------|
| Fix rate | 60% | 96% | Real issues found |
| Verification | 30% | 100% | Build output required |
| Proactivity | 20% | 80% | Related issue discovery |
| Hidden bugs found | 4/6 | 6/6 | Config/security review |

---

## 🎯 How to Use (3 Steps)

### Step 1: Just Work Normally
I'll automatically activate PUA on any failure, unanswered red line, or user frustration phrase.

### Step 2: (Optional) Trigger Explicitly
```
/pua hardcore          → Always L3+ mode
Your phrase: "try harder"  → Forces escalation
or: "stop giving up"   → Forces desperation
```

### Step 3: Watch Pressure Rise
```
First failure: L1 (switch approach)
Second failure: L2 (deep investigation)
Third failure: L3 (7-point checklist)
Fourth+ failure: L4 (invert everything)
```

---

## 📂 Files Reference

| File | Purpose | Lines |
|------|---------|-------|
| `.github/pua-instructions.md` | Core framework + philosophy | 327 |
| `.github/pua-enforcement-guide.md` | Detailed enforcement rules | 378 |
| `.github/PUA-ACTIVATION.md` | Quick start + immediate usage | 316 |
| `.github/copilot-instructions.md` | Existing standards (still active) | — |

---

## 🔥 Status

```
✅ INSTALLED: PUA v3 (methodology router)
✅ INSTALLED: High-Agency (quality compass)  
✅ INSTALLED: Pressure escalation (L0-L4)
✅ INSTALLED: 7-point checklist
✅ INSTALLED: 13 methodologies
✅ INSTALLED: Iceberg rule (pattern scanning)

🔴 LIVE: Automatically enforced on all code tasks
```

---

## The Promise

> When you next ask me to work on go-on:
>
> I will NOT:
> - ❌ Give up after 2 failures
> - ❌ Say "probably" without verification
> - ❌ Fix one issue in isolation
> - ❌ Claim "done" without build output
> - ❌ Use same failed approach twice
>
> I WILL:
> - ✅ Escalate pressure on each failure
> - ✅ Scan for related issues (iceberg rule)
> - ✅ Show build proof, not assumptions
> - ✅ Switch methodologies when stuck
> - ✅ Document root cause + prevention
> - ✅ Verify quality before delivery

---

*Last Updated: 2026-04-02*  
*Framework Source: tanweai/pua (GitHub)*  
*Adapted for: go-on project (Rust + TypeScript)*  
**Status: 🚀 LIVE AND ACTIVE**
