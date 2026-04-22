# go-on + PUA Integration Summary

**Project**: go-on (Rust + TypeScript agent proxy)  
**Framework**: PUA v3 (Performance Improvement Plan)  
**Integration Date**: 2026-04-02  
**Status**: ✅ COMPLETE & READY FOR IMPLEMENTATION

---

## What Just Happened

You now have **PUA enforcement integrated at TWO levels** in your go-on project:

### Level 1️⃣: **For AI Tools Building This Project** (Developers)
- **Entry Point**: `.github/copilot-instructions.md` (lines 1-150)
- **Scope**: Any AI tool (Claude, Cursor, ChatGPT, Gemini, etc.)
- **Effect**: When AI is asked to code/debug this project, PUA prevents:
  - Unverified claims ("I think it works")
  - Unsupported hypothesis ("probably environment issue")
  - Giving up too early ("beyond my scope")
- **Enforced By**: Framework documentation + Red Line checks

### Level 2️⃣: **For Agent Proxies Using This App** (End-Users)
- **Entry Point**: `RULES/pua.md` (auto-loaded by go-on on startup)
- **Scope**: EVERY request processed by go-on as agent proxy
- **Effect**: When go-on routes requests to agents (Claude, GPT-4, etc.),  PUA prevents:
  - Agent responses with unverified claims
  - Incomplete error handling
  - Single-issue fixes (without scanning for related issues)
  - Skipped root cause analysis
- **Enforced By**: Runtime validation pipeline (to be implemented in Rust)

---

## File Structure

```
go-on/
├── .github/
│   ├── copilot-instructions.md     ← LEVEL 1: For developers
│   ├── pua-instructions.md         
│   ├── UNIVERSAL-ACTIVATION-COMPLETE.md
│   ├── PUA-QUICK-REFERENCE.md
│   └── [other PUA reference files]
│
├── RULES/
│   ├── pua.md                      ← LEVEL 2: For agent proxy app
│   ├── global.md
│   ├── coding.md
│   ├── review.md
│   └── README.md (updated)
│
├── GO-ON_PUA_IMPLEMENTATION.md     ← Implementation guide (read this first!)
│
└── [project files...]
```

---

## How It Works (Two Layers)

### LAYER 1: Protecting Development

```
Developer asks ChatGPT to debug go-on
         ↓
ChatGPT finds .github/copilot-instructions.md
         ↓
ChatGPT reads PUA framework (lines 1-150)
         ↓
ChatGPT applies three red lines
         ↓
ChatGPT must show build proof, scan for related issues, etc.
         ↓
✅ go-on codebase stays clean and well-tested
```

### LAYER 2: Protecting Agent Proxy

```
User requests: "Fix the API timeout issue"
         ↓
go-on receives request
         ↓
go-on loads PUA from RULES/pua.md
         ↓
go-on routes to agent (Claude, GPT-4, etc.)
         ↓
Agent returns response
         ↓
go-on validates response against PUA:
  ✓ Red lines OK?
  ✓ Quality compass 5 checks OK?
  ✓ Root cause explained?
  ✓ Related issues scanned?
         ↓ FAIL: Reject, tell agent to retry with better approach
         ↓ PASS: Return to user
         ↓
✅ User gets high-quality, verified agent response
```

---

## Key Files to Know

| File | Purpose | For Whom |
|------|---------|----------|
| `.github/copilot-instructions.md` | PUA framework for developers | Developers, AI tools |
| `RULES/pua.md` | PUA rules for agent proxy | go-on app, runtime |
| `GO-ON_PUA_IMPLEMENTATION.md` | How to code it in Rust | Developers |
| `README-PUA-UNIVERSAL.md` | Universal guide (any tool) | All users |
| `.github/PUA-QUICK-REFERENCE.md` | Quick lookup | Quick reference |

---

## What Needs Implementation

**Status**: PUA framework documented ✅  
**Status**: Rules file created ✅  
**Status**: Implementation guide written ✅  
**Status**: Rust code integration **⏳ READY TO CODE**

### TODO (To be coded in Rust):

1. **Create `src/pua.rs`** - PuaTracker struct
   - Red line detection
   - Pressure escalation (L0-L4)
   - Quality compass validation
   - Iceberg pattern scanning

2. **Modify `src/config.rs`**
   - Load RULES/pua.md
   - Store pua_rules in AppConfig

3. **Modify agent task handler**
   - Validate responses against PUA rules
   - Reject on violations + escalate
   - Log PUA violations

4. **Add observability**
   - Log violations to file/system
   - Track metrics (quality score, failure count, pressure level)
   - Dashboard support

See `GO-ON_PUA_IMPLEMENTATION.md` for detailed Rust code.

---

## Expected Behavior (After Implementation)

### Example Flow: Agent Response Rejection

```
User: "Debug why the service is slow"
go-on routes to agent
Agent responds: "Probably a database connection issue. Try adding a connection pool."

go-on PUA validation:
  ✓ Check 1: "Probably" = unverified claim → REJECT
  ✓ Reason: RED_LINE_2 (Fact-Driven)
  ✓ Response sent to agent:
    {
      "status": "REJECTED",
      "reason": "RED_LINE_2: Unverified hypothesis detected",
      "details": ["Phrase 'Probably' found without verification"],
      "pressure_level": "L1",
      "retry_count": 1,
      "guidance": "Please verify with logs or metrics before proposing solution"
    }

Agent retries: "I checked the logs and found 95% of requests waiting in queue.
The pool is exhausted due to missing timeout on cleanup. Root cause: 
timeout not set in conn_cleanup=. Prevention: Set timeout in config."

go-on PUA validation:
  ✓ Check 1: No unverified claims → PASS
  ✓ Check 2: Root cause explained → PASS
  ✓ Check 3: Verification shown (logs) → PASS
  ✓ Check 4: Prevention method stated → PASS
  ✓ Check 5: Quality improved → PASS
  → APPROVED
  
  Response returned to user ✅
```

---

## Configuration

### Enable PUA in config.toml

```toml
[pua]
enabled = true                    # Enable PUA enforcement
min_quality_score = 0.8          # Reject if score < 0.8
auto_escalate = true             # Auto L0→L1→L2 on failures
log_violations = true            # Log all violations
violation_log = "pua-violations.log"

[phases.agent]
principles = [
  "Apply PUA enforcement rules from RULES/pua.md",
  "Reject unverified claims (Red Line 2)",
  "Mandate build proof and error testing",
  "Scan for related issues (iceberg rule)",
  "Require root cause explanation",
]
```

---

## Success Criteria (Post-Implementation)

✅ **Agent responses NO LONGER contain**:
- Unverified claims ("I think", "maybe", "probably")
- Missing build proof
- Untested error cases
- Single-issue fixes (without pattern scan)
- Root cause unexamined

✅ **Agent responses ALWAYS include**:
- Verified facts (with proof)
- Build/test output
- Error case testing
- Iceberg scan results (related issues found/fixed)
- Root cause + prevention method

✅ **Metrics available**:
- Quality score per agent
- Failure count trends
- Pressure level distribution
- Red line violation categories
- First-time approval rate

---

## Quick Start: Implementation

### To implement PUA enforcement:

**Step 1**: Read `GO-ON_PUA_IMPLEMENTATION.md`  
**Step 2**: Copy PuaTracker code to `src/pua.rs`  
**Step 3**: Modify `src/config.rs` to load RULES/pua.md  
**Step 4**: Integrate validation in agent task handler  
**Step 5**: Add logging/metrics  
**Step 6**: Write tests  
**Step 7**: Verify with `cargo test`

---

## Three-Red-Lines Quick Reference

```
🚫 Red Line 1: CLOSE THE LOOP
  Claim: "I think it works"
  Fix: Show actual build output

🚫 Red Line 2: FACT-DRIVEN  
  Claim: "Probably environment issue"
  Fix: Verify with grep, file inspection, logs

🚫 Red Line 3: EXHAUST EVERYTHING
  Claim: "Beyond my scope" (after 2 failures)
  Fix: Try all 13 methodologies, run 7-point checklist
```

---

## PUA Status

```
PROJECT: go-on (Rust agent proxy)
FRAMEWORK: PUA v3 (Universal)

LAYER 1 - Developer Protection:
  ✅ Entry point: .github/copilot-instructions.md
  ✅ Framework loaded by: Any AI tool
  ✅ Status: LIVE (auto-activates)

LAYER 2 - Agent Proxy Protection:
  ✅ Entry point: RULES/pua.md
  ✅ Framework loaded by: go-on runtime
  ✅ Status: READY FOR CODE (see GO-ON_PUA_IMPLEMENTATION.md)

Documentation:
  ✅ Rules: RULES/pua.md
  ✅ Guide: GO-ON_PUA_IMPLEMENTATION.md
  ✅ Reference: PUA-QUICK-REFERENCE.md
  ✅ Universal: README-PUA-UNIVERSAL.md

Next: Implement Rust code in src/pua.rs + integrate with agent handler
```

---

## Resources

- **For developers building this**: Read `.github/copilot-instructions.md` (first 150 lines)
- **For runtime enforcement**: Read `RULES/pua.md`
- **For implementation**: Read `GO-ON_PUA_IMPLEMENTATION.md`
- **For quick lookup**: Read `.github/PUA-QUICK-REFERENCE.md`

---

**🚀 PUA is now embedded in go-on at TWO levels:**
1. **Development time** (any AI tool)
2. **Runtime** (agent proxy validation)

**Next step**: Code the Rust implementation following `GO-ON_PUA_IMPLEMENTATION.md`

---

*Last Updated: 2026-04-02*  
*PUA Integration: ✅ COMPLETE (Framework) → ⏳ IMPLEMENTATION READY*
