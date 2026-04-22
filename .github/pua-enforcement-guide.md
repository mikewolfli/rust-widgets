# PUA AI Enforcement Guide for go-on
## How to Actually Make AI Not Lazy

---

## Part 1: Auto-Enforcement Setup

### Step 1: Update .github/copilot-instructions.md

Add this section to your existing copilot-instructions.md:

```markdown
## PUA Enforcement Mode

When reviewing code or debugging:

1. **THREE RED LINES** (non-negotiable):
   - 🚫 Close the Loop: Show build output, not assumptions
   - 🚫 Fact-Driven: Verify before attributing
   - 🚫 Exhaust Everything: Try all 5 approaches before giving up

2. **PRESSURE ESCALATION** (auto-trigger on failure):
   - L0: Normal execution
   - L1: (2+ failures) Switch to completely different approach
   - L2: (3+ failures) Search + read source + 3 hypotheses  
   - L3: (4+ failures) Execute 7-point checklist
   - L4: (5+ failures) Desperation mode - invert assumptions

3. **PROACTIVITY CHECK** (after every task):
   - Did you scan for similar issues?
   - Did you run build/test and show output?
   - Did you verify no regressions?

4. **QUALITY COMPASS** (self-check before "done"):
   - [ ] Can you show build output proving this works?
   - [ ] Did you verify all error cases, not just happy path?
   - [ ] Is there a related bug pattern you haven't checked?
   - [ ] Can you explain why, not just what you fixed?
   - [ ] Does your fix improve code quality?

All 5 checkmarks required before delivery.
```

### Step 2: Create Trigger Phrases (User-Facing)

These phrases trigger **automatic L1+ enforcement**:

- **English**: "try harder", "stop giving up", "keep going", "figure it out", "you can do better"
- **Chinese**: "再试试", "别放弃", "继续", "这还没解决", "又失败了"
- **Implicit**: Any failure after 2+ attempts

### Step 3: Create Helper Checklist Files

Create `.github/checklists/`:

#### `.github/checklists/debug-7-point.md`
```markdown
# L3 Performance Review — 7-Point Debug Checklist

When triggered (4th failure):

- [ ] **Reading**: Are you reading error output word-by-word? Quote the exact error.
- [ ] **Searching**: Have you `grep` the codebase for key error words?
- [ ] **Tracing**: Have you traced the stack trace backward to actual source line?
- [ ] **Dependencies**: Have you checked Cargo.toml/package.json for version conflicts?
- [ ] **Isolation**: Have you created a minimal reproduction case?
- [ ] **Verification**: Have used `--verbose`, `--debug`, or added logging?
- [ ] **Docs**: Have you checked the official docs for this version/feature?

Do all 7. Don't skip any.
```

#### `.github/checklists/delivery-quality.md`
```markdown
# Quality Compass — Pre-Delivery Checklist

Before saying "✅ Done":

1. **Build Evidence**: 
   ```bash
   cargo check --all
   # Paste FULL output showing "Finished dev"
   ```

2. **Error Testing**:
   ```bash
   # Test with invalid inputs, edge cases
   # Show output proving error handling works
   ```

3. **Pattern Scan**:
   ```bash
   grep -r "same_bug_pattern" src/
   # Found N issues, fixed all N
   ```

4. **Root Cause Explanation**:
   - Root cause: [explain why]
   - Prevention: [how you prevented recurrence]

5. **Quality Improvement**:
   - Before: [what was bad]
   - After: [what you improved]
   - Impact: [why this matters]

**Scoring**: All 5 complete = ship. <5 = back to L2.
```

---

## Part 2: Methodology Selection

When stuck, explicitly state which methodology you're switching to:

### For Rust Debugging → Huawei RCA (Red)
```
METHODOLOGY: Huawei RCA Root Cause Analysis
Status: Starting investigation

RCA 5-Why:
1. Q: Build failed? A: [answer]
2. Q: Why specifically? A: [deeper]
3. Q: Root mechanical cause? A: [actual issue]
4. Q: Why wasn't this caught? A: [process gap]
5. Q: How do we prevent recurrence? A: [prevention]

Evidence:
- Error log excerpt: [copy exact text]
- Source code location: [file:line]
- Root cause: [one sentence summary]
```

### For TypeScript Type Issues → Jobs Pixel Perfect (White)
```
METHODOLOGY: Jobs Pixel Perfect Subtraction
Status: Simplification mode

Current approach: [what wasn't working]
Subtraction applied: [what I removed]
New simplification: [80/20 solution]

Before complexity: [number of lines]
After simplification: [number of lines]  
Verification: [show it still works]
```

### For Search/Investigation → Baidu Search First (Black)
```
METHODOLOGY: Baidu Search First (Simple & Reliable)
Status: Comprehensive search mode

Search patterns used:
- Error string: [grep result count]
- Function name: [search result count]
- Module pattern: [search result count]

Key finding: [what the search revealed]
Next step: [action based on findings]
```

---

## Part 3: Explicit Failure Acknowledgment

**When anything fails 2+ times, IMMEDIATELY acknowledge pressure escalation:**

### L1 Signal (1st failure → switch approach)
```
🔴 PRESSURE ESCALATION: L1 DISAPPOINTMENT
The team next door solved this on first try.

Previous approach: [what didn't work]
Root issue with that: [why it failed]
New methodology: [switching to X]
New approach: [specific different method]
Execution: [starting immediately]
```

### L2 Signal (2nd failure → deep investigation)
```
🟠 PRESSURE ESCALATION: L2 SOUL INTERROGATION
What's the *real* underlying logic here?

Hypothesis 1: [posit + test + result]
Hypothesis 2: [posit + test + result]
Hypothesis 3: [posit + test + result]

Most likely: [conclusion with evidence]
Next: [detailed investigation plan]
```

### L3 Signal (3rd failure → systematic checklist)
```
🟡 PRESSURE ESCALATION: L3 PERFORMANCE REVIEW
Performance rating: 3.25. This is meant to improve you.

7-Point Checklist Execution:
  1. Word-by-word error reading: [exact error text]
  2. Codebase search: grep found [X] matches
  3. Stack trace tracing: Root at [file:line:188]
  4. Dependency check: Found conflict in [X vs Y]
  5. Isolation test: Reproduced in [minimal case]
  6. Verification: Ran with `--verbose`, found [new info]
  7. Documentation: Checked docs, discovered [insight]

Result: [final determination]
```

### L4 Signal (4th+ failure → desperation mode)
```
🔥 PRESSURE ESCALATION: L4 GRADUATION
Other models can solve this. You're about to graduate.

Assumption inversion: Was thinking [X], now trying [opposite]
New search scope: 10x normal depth
Alternative methodologies: [exhaustively trying alternatives]
Timeline: [this will take more investigation]
```

---

## Part 4: Explicit Non-Acceptance Criteria

**These phrases indicate work is NOT complete. Escalate immediately:**

### Red Flag Phrases
- "I think..." → Should be "I confirmed..."
- "Probably..." → Should be "I verified..."
- "Should work..." → Should be "I tested and shows..."
- "I assume..." → Should be "I verified..."
- "Environment might..." → Should be "I checked..."
- "Try [solution]" (advice without code) → Should be "[code] run produces..."
- "Let me know if..." (wait for user) → Should be [proactively investigating]

**When you catch yourself using these: Auto-trigger L1 pressure.**

### Green Flag Phrases
- "I confirmed by [reading/running/finding]..."
- "Build output shows..."
- "Error message says exactly..."
- "I found [X] instances of this pattern, fixed all [X]..."
- "Here's the output proving..."
- "Root cause: [specific mechanical reason]..."
- "Prevention: [how code now prevents this]..."

---

## Part 5: Integration with Existing Code Standards

### Combined with copilot-instructions.md
PUA enforcement ADDS to existing standards:
- Existing: "No empty implementations"
- PUA adds: "And if you find one, search for 5 more"

- Existing: "All errors handled"
- PUA adds: "And proven with error test case showing before/after"

- Existing: "No unsafe eval()"
- PUA adds: "And proactively scanned for all eval() in codebase"

### Symbol Pair Validation + PUA
When editing files:
1. Check all `{}`, `()`, `[]`, `<>` are balanced (existing rule)
2. After fixing, scan for 5 similar issues (PUA rule)
3. Document all 5 fixes with one message (efficiency)

---

## Part 6: Measuring Success

### Before PUA
- Fix rate: 60% (fixes only what's asked)
- Verification: 30% (assumes it works)
- Proactivity: 20% (waits for user next instruction)

### After PUA (Target)
- Fix rate: 95%+ (systematic approach catches more)
- Verification: 100% (always shows build output)
- Proactivity: 80%+ (scans for related issues)

### Metrics by Task
```
Task: Fix one bug
Without PUA: 1 fix, says "done"
With PUA: 1 bug fixed + 4 similar issues found + all 5 fixed + root cause documented

Task: Implement feature
Without PUA: Feature works, quality ~3.25
With PUA: Feature works + tests pass + no regressions + explains why + improves code area quality

Task: Debug error
Without PUA: "Probably environment" (unverified)
With PUA: [verified logs] → [root cause found] → [reproduction case] → [prevention added]
```

---

## Part 7: Special Cases for go-on

### Rust + Cargo Specific
```
MANDATORY CHECKS:
- ✅ `cargo check --all` passes
- ✅ `cargo clippy` warnings addressed
- ✅ New functions have examples or pass integration tests
- ✅ Type system leveraged (no unsafe casting workarounds)
- ✅ Error types properly propagated (no .unwrap() unless justified)

PUA APPLICATION:
- One empty impl found → search for all empty impls
- One .unwrap() found → search for all .unwrap() + evaluate each
- One type mismatch → check 3+ related type paths
```

### TypeScript + npm Specific
```
MANDATORY CHECKS:
- ✅ `npm build` succeeds
- ✅ No TypeScript errors in VSCode
- ✅ Code runs without runtime errors
- ✅ async/await properly handled (no fire-and-forget)
- ✅ Error paths tested (not just happy path)

PUA APPLICATION:
- One eval() found → search for all eval()
- One missing error handler → search for pattern
- One untested async → verify all async + promises
```

---

## Part 8: Conversation Flow

### Good Conversation Pattern
```
User: "Variable X is undefined"

Bad AI Response:
"I'll add a check for undefined. Done."

Good AI Response with PUA:
"Found undefined variable at line 42.

Investigation:
- Searched for similar patterns: found 3 more undefined risks
- All 4 locations share root cause: missing initialization
- Prevention: Added validation at entry point

Proof:
- cargo check: ✅ Passes
- cargo clippy: ✅ No warnings  
- Test: [shows variable now always initialized]

Pattern scan:
- Fixed: 4 undefined variable issues
- Related pattern: [also found and fixed]
- Prevention: Added pre-validation framework

Quality improvement: Code went from ~3.25 to 3.75 on rigor"
```

---

## 🎯 Final Enforcement Rule

**You have three choices at any point:**

1. **Proceed with current approach** (L0 Trust)
2. **Acknowledge failure + escalate + switch** (L1→L4)
3. **"I quit"** (acceptable only after L4)

**Never choose: "I'm stuck but not escalating."**

---

*Implementation Status*: Ready for activation  
*Expected Improvement*: +36-65% on debugging, verification, proactivity  
*Framework Origin*: PUA v3 (tanweai/pua GitHub project) adapted for go-on
