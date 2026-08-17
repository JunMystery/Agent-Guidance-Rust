# Upgrade Proposal 04: Project Learnings & Cross-Agent Handoff

> **Target Tool**: `session_continuity`
> **Primary Purpose**: Biến phiên làm việc của Agent thành bộ nhớ kinh nghiệm lâu dài và hỗ trợ chuyển giao phiên làm việc mượt mà giữa các AI Agent khác nhau.

---

## 1. Vấn đề Hiện tại (Problem Statement)
- Session state hiện tại lưu dạng file JSON kỹ thuật (`session_*.json`), chủ yếu để ghi nhớ tool calls và workflow gates.
- Khi kết thúc phiên làm việc, các bài học quan trọng (ví dụ: các cạm bẫy, cấu hình môi trường đặc thù, các câu lệnh test riêng) bị mất đi. Ở phiên làm việc sau hoặc ngày hôm sau, Agent mới lại dễ mắc lại sai lầm cũ.
- Khi người dùng chuyển đổi giữa các IDE (ví dụ: đang dùng VS Code / Claude Code chuyển sang Antigravity / Cursor), Agent mới phải mất công dò tìm lại từ đầu.

---

## 2. Giải pháp Đề xuất (Proposed Solution)

### A. Automatic Project Knowledge Distillation (Đúc kết tri thức dự án)
- Khi một task vượt qua stage `Test_Recheck` và hoàn tất thành công, `session_continuity` tự động tổng kết 2–3 bài học cốt lõi:
  - *"Dự án này chạy test bằng lệnh `cargo test --all-features`"*.
  - *"Module database sử dụng schema migration tại `migrations/` thay vì tự tạo bảng"*.
- Lưu trữ vào `.agent-context/learnings.md` và tự động nạp vào phần context tóm tắt cho các phiên làm việc tiếp theo.

### B. Instant Cross-Agent Handoff Protocol (Biên bản bàn giao giữa các Agent)
- Cung cấp operation `session_continuity(operation="handoff")`:
  - Trả về bản tóm tắt 3 dòng siêu ngắn:
    1. **Mục tiêu task vừa làm**: Đang giải quyết tính năng gì.
    2. **File đã thay đổi**: Danh sách các file vừa sửa và trạng thái kiểm thử.
    3. **Bước tiếp theo cần làm ngay**: Gợi ý hành động tiếp theo cho Agent mới.
  - Cho phép chuyển đổi qua lại giữa Cursor, Claude Code, Windsurf, Antigravity mà không bị đứt gãy mạch suy nghĩ.

---

## 3. Tác động Dự kiến (Expected Impact)
- Xây dựng "trí nhớ tổ chức" cho từng dự án, Agent càng làm việc lâu thì càng thông minh và ít mắc lỗi.
- Hỗ trợ làm việc mượt mà trên môi trường Multi-IDE và Multi-Agent.
