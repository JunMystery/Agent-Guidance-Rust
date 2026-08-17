# Danh mục Đề xuất Nâng cấp MCP Server (Upgrade Proposals Index)

Thư mục này chứa các bản tóm tắt (brief proposals) cho lộ trình nâng cấp các công cụ cốt lõi trong hệ sinh thái **Agent Guidance MCP Server**.

---

## 📑 Danh sách các bản Đề xuất

| Mã | Bản đề xuất | Công cụ mục tiêu | Tác động chính |
|---|---|---|---|
| **01** | [Predictive Stage Transition & Diff Impact Guard](01-workflow-gate-predictive-impact-guard.md) | `workflow_gate` | Tự động chuyển stage theo ý định, cảnh báo sửa đổi các file Core Hub rủi ro cao. |
| **02** | [Dynamic Blueprint Synthesis](02-task-pipeline-dynamic-blueprint.md) | `task_pipeline` | Sinh bản thiết kế chia tách file thực tế dựa trên Code Graph, gom gói Skill Recipes. |
| **03** | [Context-Adaptive Skill Pruning & Micro-Guidance](03-skill-pruning-micro-guidance.md) | `guidance`, `select_skills` | Dùng Multilingual-E5 cắt lát tài liệu skill (giảm 70% token), cảnh báo lỗi theo ngôn ngữ. |
| **04** | [Project Learnings & Cross-Agent Handoff](04-knowledge-distillation-handoff.md) | `session_continuity` | Đúc kết kinh nghiệm dự án lâu dài (`learnings.md`), chuyển giao phiên làm việc giữa các IDE. |
| **05** | [AST Structural Skeletonization](05-ast-structural-skeletonization.md) | `optimizer`, `project_context` | Rút gọn file code lớn thành khung chữ ký hàm/struct (giảm 90% token khi xem file lớn). |

---

## 🎯 Thứ tự ưu tiên & Kế hoạch thảo luận
Mỗi đề xuất được thiết kế hoàn toàn độc lập, giải quyết các điểm nghẽn riêng biệt của từng công cụ mà không làm cồng kềnh hệ thống.
