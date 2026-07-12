# 附录 B 场景 DSL 参考卡

基于 agent-spec 1.0.0。合同段落头（每行恰好一个，不混排双语）：

| 中文 | English |
|------|---------|
| `## 意图` | `## Intent` |
| `## 约束` | `## Constraints` |
| `## 已定决策` / `## 决策` | `## Decisions` |
| `## 边界` | `## Boundaries` |
| `## 验收标准` / `## 完成条件` | `## Acceptance Criteria` / `## Completion Criteria` |
| `## 排除范围` | `## Out of Scope` |
| `## 问题` / `## 待澄清` | `## Questions` |

## 场景骨架

```markdown
### Rule: kebab-case-id — 显示名（可改，id 不动）
场景: 名称（critical）          # 名称后缀=critical 简写
  标签: critical
  测试:
    包: my-crate
    过滤: test_name
    级别: integration
  前置: 另一场景名
  审核: human                    # 测试过 → pending_review
  模式: optimize                 # 通过进优化候选,失败仍阻断
  假设 前置条件:
    | 列1 | 列2 |
    | a   | b   |
  当 动作
  那么 确定性结果（"返回 201"，不写"应该返回"）
  并且 补充断言
  但是 反向断言
```

## 边界子节

```markdown
## Boundaries

### Allowed Changes      # 机械执法的路径 glob
- src/auth/**

### Forbidden            # lint 检查的自然语言禁令
- 不要添加新依赖

### Symbols              # 1.0:代码符号引用(Linker)
- rust-atlas: crate::module::Item

## Out of Scope
- 显式排除项
```

## frontmatter

```yaml
spec: task            # org | project | task
name: "名称"
inherits: project     # 三层继承链 org→project→task
tags: [feature]
satisfies: [REQ-X]    # KLL satisfies 边
depends: [task-a]     # 合同依赖(graph 用)
estimate: 2d          # 0.5d/1d/2d/1w/4h
capability: cap-name  # 贡献到哪个能力
risk: A               # QA class
---
```

## lint-ack

```markdown
<!-- lint-ack: error-path — 只读查询无失败路径 -->
```

码与理由必须用 `—` 或 `:` 分隔；Error 永不可豁免；豁免计入 audit 台账。
