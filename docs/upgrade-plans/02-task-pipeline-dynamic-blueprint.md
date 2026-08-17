# Upgrade Proposal 02: Dynamic Blueprint Synthesis

> **Target Tool**: `task_pipeline`
> **Primary Purpose**: Nâng cấp bản thiết kế phân tách code từ dạng lý thuyết chung chung sang bản thiết kế chính xác theo cấu trúc mã nguồn thực tế của dự án.

---

## 1. Vấn đề Hiện tại (Problem Statement)
- `task_pipeline` hiện tại trả về các quy chuẩn kiến trúc dạng khuôn mẫu (ví dụ: *"Layered Architecture: controllers -> services -> models"* hoặc *"300 LOC Cap"*).
- Agent vẫn phải tự đọc code, tự tìm các file đang phình to để suy nghĩ cách chia tách, dẫn đến việc mất thêm 2–3 turn lập kế hoạch.

---

## 2. Giải pháp Đề xuất (Proposed Solution)

### A. Graph-Driven Split Blueprint (Chỉ dẫn phân tách theo file thực tế)
- `task_pipeline` đọc dữ liệu từ `code_graph.db` để phát hiện các file đang vi phạm hoặc sắp chạm ngưỡng 300 LOC trong domain liên quan đến task.
- Tự động sinh bản thiết kế cụ thể:
  ```markdown
  ### 📐 Upfront Split Blueprint cho Task:
  - File hiện tại: `src/auth/handler.rs` (380 LOC — Vi phạm Cap)
  - Đề xuất tách thành:
    1. `src/auth/handler.rs`: Giữ lại route dispatchers (< 80 LOC).
    2. `src/auth/jwt.rs`: Tách hàm `verify_token`, `generate_claims` (~120 LOC).
    3. `src/auth/password.rs`: Tách hàm `hash_password`, `validate_complexity` (~90 LOC).
  ```

### B. Skill Recipe Bundling (Gom gói kỹ năng theo kịch bản)
- Thay vì gợi ý 5–8 kỹ năng riêng lẻ dễ gây phân mảnh context, tool tự động nhận diện domain của task và tổng hợp thành **1 gói kỹ năng duy nhất** (Skill Recipe) kèm các checklist cô đọng.

---

## 3. Tác động Dự kiến (Expected Impact)
- Định hướng kiến trúc chính xác ngay từ dòng code đầu tiên (Line 1).
- Tiết kiệm 3–5 prompt trao đổi về cách bố trí và tổ chức file.
