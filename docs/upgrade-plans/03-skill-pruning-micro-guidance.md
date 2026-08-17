# Upgrade Proposal 03: Context-Adaptive Skill Pruning & Micro-Guidance

> **Target Tools**: `guidance`, `select_skills`
> **Primary Purpose**: Cắt gọt các tài liệu skill khổng lồ thành các lát cắt ngữ cảnh súc tích và tự động cảnh báo anti-pattern tức thì.

---

## 1. Vấn đề Hiện tại (Problem Statement)
- Mỗi skill tài liệu có độ dài từ 300 đến 800 dòng Markdown.
- Khi Agent gọi `select_skills`, toàn bộ tài liệu được đẩy vào Context Window $\rightarrow$ Làm đầy context nhanh chóng và khiến Agent bị loãng thông tin, đôi khi bỏ qua các chỉ dẫn quan trọng nhất.

---

## 2. Giải pháp Đề xuất (Proposed Solution)

### A. Semantic Skill Slicing (Cắt lát kỹ năng thông minh)
- Tận dụng cỗ máy embedding Multilingual-E5 có sẵn trong `src/ml/`:
  - Khi Agent yêu cầu nạp skill với một `task` cụ thể (ví dụ: *"xử lý timeout kết nối"*), tool thực hiện embedding câu hỏi và tính cosine similarity trên từng tiêu đề/đoạn văn của skill đó.
  - Chỉ trích xuất và trả về 2–3 section có độ liên quan cao nhất (ví dụ: phần *"Timeout Configuration"* & *"Retry Backoff Policy"*), loại bỏ các phần râu ria.
  - **Giảm ngay 60–75% lượng token tiêu thụ** cho việc nạp tài liệu.

### B. Micro-Guidance Injection (Cảnh báo chống lỗi đặc thù theo ngôn ngữ)
- Nhận diện ngôn ngữ lập trình của project để tự động đính kèm 3 quy tắc "sống còn" ngắn gọn:
  - **Rust**: Không dùng `.unwrap()`, ưu tiên `?`, hạn chế `.clone()` không cần thiết.
  - **TypeScript**: Bắt buộc dùng `unknown` thay vì `any`, xử lý `null`/`undefined` an toàn.
  - **Python**: Bắt buộc Type Hinting, context manager `with` cho tài nguyên I/O.

---

## 3. Tác động Dự kiến (Expected Impact)
- Giảm tải cực lớn cho Context Window của Agent.
- Tăng độ tập trung và mức độ tuân thủ quy chuẩn kỹ thuật của Agent.
