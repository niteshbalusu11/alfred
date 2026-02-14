# GitHub Issue Update Template

Use this template for consistent issue updates during execution.

## 1) Work Started Comment

```md
Starting work on this issue.

Branch: `codex/issue-<number>-<slug>`
Scope: <short scope statement tied to acceptance criteria>
Plan:
1. <step 1>
2. <step 2>
3. <step 3>
```

## 2) Progress Update Comment

```md
Progress update:

Completed:
- <completed item>
- <completed item>

In progress:
- <in-progress item>

Next:
- <next step>

Risks/notes:
- <risk or note>
```

## 3) Blocked Update Comment

```md
Blocked.

Blocker:
- <what is blocked and where>

Why blocked:
- <root cause>

What is needed:
- <decision/input/access/dependency>

Proposed fallback:
- <fallback option>
```

## 4) Ready for Review / Done Comment

```md
Completed and ready for review.

What changed:
- <change 1>
- <change 2>
- <change 3>

Acceptance criteria mapping:
- [x] <criterion 1>
- [x] <criterion 2>
- [x] <criterion 3>

Validation:
- `just backend-check` => <pass/fail>
- `just ios-build` => <pass/fail>
- `just backend-test` => <pass/fail or n/a>
- `just backend-deep-review` => <pass/fail or n/a>

Follow-ups:
- <none or explicit follow-up>

Deep review findings:
- <finding or `no findings`>

AI review summary:
- Security audit: <findings or `no findings`>
- Bug check: <findings or `no findings`>
- Scalability/code quality review: <findings or `no findings`>
- Merge recommendation: <APPROVE/BLOCK>
```

## 5) Optional PR Description Template

```md
## Summary
<short summary>

## Changes
- <change>
- <change>

## Validation
- `just backend-check`
- `just ios-build`
- `just backend-test` (if backend behavior changed)

## Issue
Closes #<issue-number>
```

## 6) Usage Rules

1. Keep updates tied to issue acceptance criteria.
2. Post a `Work Started` comment before meaningful code changes.
3. Post a `Blocked` comment immediately when unable to continue.
4. Post `Ready for Review / Done` with command results before closing.
