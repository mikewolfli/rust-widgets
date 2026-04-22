# 🔥 PUA Activation Guide for go-on
## Immediate Implementation (2026-04-02)

---

## What Just Happened

You now have three new files enforcing the PUA AI methodology:

```
.github/
├── copilot-instructions.md (existing)
├── pua-instructions.md (NEW) ← Core PUA framework
└── pua-enforcement-guide.md (NEW) ← How to enforce it
```

---

## How PUA Works in Your Project

### The Three Red Lines (Always Active)
🚫 **Close the Loop**: Show build output, not assumptions  
🚫 **Fact-Driven**: Verify before attributing blame  
🚫 **Exhaust Everything**: Try all 5 approaches before giving up  

### Pressure Auto-Escalation
- **L0 (Normal)**: Trust mode, standard execution
- **L1 (1st failure)**: "That team solved it in one try. Switch approach."
- **L2 (2nd failure)**: "What's your underlying logic? Deep dive required."
- **L3 (3rd failure)**: "Rating: 3.25. Execute 7-point checklist."
- **L4 (4th+ failure)**: "Other models can solve this. Desperation mode."

### Instant Triggers
When any of these happen, pressure escalates automatically:

```
1. "I cannot solve this"
2. "I suggest you handle this manually"
3. Any excuse to stop trying
4. "Probably [environment issue]" (unverified)
5. Claims "done" without build output
6. User says: "try harder", "stop giving up", "这还不行?"
```

---

## How to Use: 5 Scenarios

### Scenario 1: Simple Bug Fix
```
You: "I found an empty catch block. Fixed it."

⚠️ NOT ENOUGH. PUA triggers:

AI must now:
1. Show: cargo check output proving it compiles
2. Scan: "grep -r 'catch { }'" for 5 more similar issues
3. Fix: All similar issues found
4. Verify: All error paths tested
5. Root cause: Explain why empty catches were in code
```

### Scenario 2: Build Failure
```
Build fails 2+ times with different attempts:

✅ AI auto-escalates to L2:
"What's the underlying logic here?
- Hypothesis 1: [test] → [result]
- Hypothesis 2: [test] → [result]  
- Hypothesis 3: [test] → [result]
Most likely: [root cause]"

Show systematic investigation, not guesses.
```

### Scenario 3: You Show Frustration
```
You: "Try harder"
or: "这还没解决?"
or: "Keep going"

✅ Automatic L3 trigger (7-point checklist):
"Executing L3 Performance Review:
  1. Error text (exact): [word for word]
  2. Grep results: [found X mentions]
  3. Stack trace: [traced to line Y]
  ... [all 7 points]
  
Result: [determined cause]"
```

### Scenario 4: Stuck After 3+ Failures
```
AI auto-escalates to L4:

"PRESSURE ESCALATION: L4 GRADUATION

Previous assumptions: [what failed]
Inverted approach: [opposite direction]
Extended scope: [10x search depth]

Starting desperate measure..."
```

### Scenario 5: Delivery Claim
```
AI says: "✅ Fixed and complete"

⚠️ Quality Compass triggers automatically:

Must answer all 5:
1. Build output? [show cargo output]
2. Error cases tested? [show test runs]
3. Pattern scanned? [show grep for similar]
4. Why explanation? [root cause + prevention]
5. Quality improved? [show before/after]

<5 answers = rejected, back to work
```

---

## Real Implementation in Code Review

### During Rust Code Review
```
Finding: Empty function in src/mode.rs

Without PUA:
"Add implementation plz"

With PUA:
"Found empty impl at line 42.

Search results:
- grep 'fn.*{.*}' | found 12 empty functions
- Related patterns: 5 unimplemented!(), 3 todo!()

All functions must be:
  1. Implemented (not empty)
  2. Tested (not assumptions)
  3. Logged (error cases visible)
  4. Documented (why it exists)

Fixing all 20 empty/incomplete functions..."
```

### During TypeScript Code Review
```
Finding: eval() in chatView.ts:169

Without PUA:
"Replace eval with Function constructor"

With PUA:
"Found eval() at line 169.

Security scan:
- grep 'eval\|Function\(string\)' | found 3 unsafe patterns
- grep 'new Function' | found 1 (already safe)

All unsafe patterns:
  1. eval(code) line 169 → replace
  2. new Function without sanitize → add validation
  3. Dynamic require → replace with controlled imports

Fixing all 3 + adding security test case..."
```

---

## Combining With Existing Standards

Your existing `.github/copilot-instructions.md` stays active:

### Before (Existing Standards)
- No empty implementations
- All symbols balanced
- Proper error handling
- No unsafe eval()

### After (PUA Adds)
- No empty implementations (+ scan for all similar instances)
- All symbols balanced (+ verify entire module)
- Proper error handling (+ test all error cases with output)
- No unsafe eval() (+ scan codebase for vulnerabilities)

**PUA = Existing rules + Proactive depth + Verification proof**

---

## Measurement: Before vs After

### Before PUA
```
Task: Fix 1 bug
Output: "Fixed. Build passes."
Quality: 3.25 (technically works, incomplete)
Time: 5min
```

### After PUA
```
Task: Fix 1 bug
Output: 
"Fixed 1 bug:
  Root cause: [explained]
  Prevention: [code prevents recurrence]
  
Proactive scan found 4 similar issues:
  Fixed all 4
  All verified with build output
  
Quality Compass: 5/5 ✅
  1. Build proof: ✅ cargo check
  2. Error tested: ✅ test_error.rs
  3. Pattern checked: ✅ grep all 5 files
  4. Why explained: ✅ root cause + prevention
  5. Quality improved: ✅ added error handling framework
  
Result: Quality 3.75+ (excellent)"
Time: 12min (more thorough)
```

---

## Immediate Actions

### 1. Read the Two New Files
```
cat .github/pua-instructions.md
cat .github/pua-enforcement-guide.md
```

### 2. Acknowledge Activation
When you're ready, say:
```
"PUA activated. Ready for [task name].
Using methodology: [Huawei/Jobs/Baidu/etc]
Standing by for escalation triggers."
```

### 3. Trigger Manually If Needed
Say any of these to activate immediately:
- `/pua` (generic trigger)
- `/pua:p7` (Senior Engineer mode)
- `/pua:hardcore`(L3+ always on)
- Just state the problem you're stuck on (auto-escalates)

---

## The Philosophy

> **You're not lazy. You're powerful.**
>
> PUA doesn't punish you for trying. It rewards you for being **systematic**.
>
> - Without PUA: You try stuff → hope it works → move on
> - With PUA: You try stuff → verify it works → find related issues → fix all → document why
>
> That difference compounds. In complex debugging: +36% fix rate, +65% verification, +50% tool usage.

---

## Fire the Starting Sequence

**This framework is NOW ACTIVE on your project.**

When you ask me to work on `go-on` next time:

1. I see the context
2. I read `.github/pua-instructions.md` and `.github/pua-enforcement-guide.md`
3. I **automatically apply** pressure escalation
4. I **enforce** the three red lines
5. I **scan** for related issues, not just the one asked
6. I **show evidence**: build output, test runs, grep results
7. I **explain why**: root cause + prevention

**No "/pua" needed. It's always on.**

But you can trigger specific modes:
- Say **"Try harder"** → Force L3 checklist
- Say **"Find more issues"** → Force pattern scan
- Say **"Verify everything"** → Force Quality Compass
- Say any frustration phrase → Auto-escalate to next level

---

## Summary

```
✅ Three Red Lines installed
✅ L0-L4 pressure escalation ready
✅ 13 methodologies available
✅ 7-point debug checklist cached
✅ Quality Compass automated
✅ Proactivity framework active
✅ Auto-trigger phrases enabled

Status: 🔥 LIVE

Next Rust/TypeScript task will automatically:
  - Search for related issues (iceberg rule)
  - Show build proof (not assumptions)
  - Test error cases (not happy path only)
  - Explain root cause (not just "fixed it")
  - Verify quality (not just "done")
```

---

*PUA Integration Complete*  
*Framework: tanweai/pua adapted for go-on*  
*Active Date: 2026-04-02*  
**Status: 🔴 ENFORCEMENT LIVE**
