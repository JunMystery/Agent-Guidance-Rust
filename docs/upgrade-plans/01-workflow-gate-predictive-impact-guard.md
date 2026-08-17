# Upgrade Proposal 01: Predictive Stage Transition & Diff Impact Guard

> **Target Tool**: `workflow_gate`
> **Primary Purpose**: Loại bỏ round-trip dư thừa cho Agent, chủ động kiểm soát rủi ro kiến trúc trước khi chỉnh sửa file trọng yếu.

---

## 1. Vấn đề Hiện tại (Problem Statement)
- **Ma sát quy trình (Workflow Friction)**: Agent thường phải mất thêm 1 turn gọi `workflow_gate` để chuyển stage thủ công (ví dụ: vừa viết xong plan $\rightarrow$ phải gọi thêm 1 turn chuyển sang `Build`).
- **Thiếu kiểm toán tác động (Blind Edits)**: Khi Agent gọi `authorize_edit`, tool chỉ kiểm tra xem stage hiện tại có phải `Build` hay không, chứ **chưa phân tích rủi ro** của file chuẩn bị sửa (ví dụ: sửa đổi một interface/core hub mà hàng chục file khác đang phụ thuộc).

---

## 2. Giải pháp Đề xuất (Proposed Solution)

### A. Zero-Turn Auto-Advance (Tự động chuyển stage theo ý định)
- Khi Agent hoàn thành artifact `implementation_plan.md` hoặc user xác nhận kế hoạch $\rightarrow$ `workflow_gate` tự động nhận diện và chuyển trạng thái sang `Build`.
- Giảm **20–30% số lượng tool calls** không cần thiết trong một phiên làm việc.

### B. Code Graph Diff Impact Guard (Đánh giá rủi ro trước khi sửa)
- Tích hợp với `code_graph.db` của `project_context`:
  - Khi Agent yêu cầu `authorize_edit` trên file `src/core/types.rs`, tool đếm số lượng node phụ thuộc vào file này trong đồ thị `symbol_edges`.
  - Nếu file có $>10$ edges phụ thuộc $\rightarrow$ Đánh dấu **HIGH RISK** và tự động kèm theo cảnh báo:
    > ⚠️ *"Cảnh báo: File này là Core Hub ảnh hưởng đến 12 module khác. Đề xuất chạy `cargo test` sau khi sửa."*

### C. Lightweight AST Snapshot & Smart Rollback
- Tự động lưu trữ snapshot hash của các file được chỉnh sửa trong phiên làm việc.
- Nếu Circuit Breaker kích hoạt ($3$ lần fix liên tiếp thất bại) $\rightarrow$ Tool cung cấp diff chính xác để hoàn tác về trạng thái ban đầu an toàn.

---

## 3. Tác động Dự kiến (Expected Impact)
- Giảm thiểu hoàn toàn nguy cơ gãy vỡ hệ thống do sửa nhầm file cốt lõi.
- Tiết kiệm 1–2 tool calls mỗi khi chuyển giai đoạn làm việc.
