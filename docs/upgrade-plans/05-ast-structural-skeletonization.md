# Upgrade Proposal 05: AST Structural Skeletonization

> **Target Tool**: `optimizer`
> **Primary Purpose**: Nén thông minh các file mã nguồn khổng lồ bằng cách giữ lại bộ khung ngữ nghĩa (Types & Signatures) và loại bỏ ruột hàm, giúp Agent nắm bắt 100% kiến trúc file mà tiêu tốn dưới 80 tokens.

---

## 1. Vấn đề Hiện tại (Problem Statement)
- Hệ thống áp dụng giới hạn cứng **300 LOC Cap** cho thao tác đọc file.
- Khi gặp một file lớn (ví dụ: 1000–2000 dòng), Agent chỉ có thể đọc 300 dòng đầu hoặc phải đọc chia nhỏ nhiều lần $\rightarrow$ Tốn rất nhiều token và không có cái nhìn toàn cảnh về cấu trúc toàn bộ file.

---

## 2. Giải pháp Đề xuất (Proposed Solution)

### A. AST Structural Skeletonization (Rút gọn khung xương code)
- Sử dụng Tree-sitter AST parser để tạo ra bản "Khung xương" của file:
  - Giữ lại toàn bộ `struct`, `enum`, `trait`, `interface`, `class` definitions.
  - Giữ lại toàn bộ chữ ký hàm (`fn name(args) -> ReturnType`), visibility (`pub`, `private`), và docstrings tóm tắt.
  - Thay thế toàn bộ thân hàm (function body) bằng `/* ... implementation ... */`.
- **Ví dụ thực tế**:
  ```rust
  // File gốc: 1,200 dòng code
  // Bản Skeleton: 45 dòng
  pub struct AuthService {
      db: Arc<DatabasePool>,
  }

  impl AuthService {
      /// Authenticates a user with email and password
      pub async fn login(&self, email: &str, password: &str) -> Result<UserToken, AuthError> { /* ... */ }

      /// Validates and refreshes JWT claims
      pub fn refresh_token(&self, token: &str) -> Result<UserToken, AuthError> { /* ... */ }
  }
  ```

### B. Tích hợp trực tiếp vào `project_context(operation="read")`
- Bổ sung parameter `view_mode: "full" | "skeleton"`:
  - Khi file $>300$ dòng, mặc định trả về bản Skeleton kèm theo vị trí dòng của từng hàm để Agent có thể chủ động đọc sâu vào hàm cụ thể cần sửa.

---

## 3. Tác động Dự kiến (Expected Impact)
- Giảm **90–95% lượng token tiêu thụ** khi Agent cần đọc hiểu các file mã nguồn lớn.
- Agent hiểu được toàn bộ API surface của file ngay lập tức mà không bao giờ bị giới hạn bởi 300 LOC Cap.
