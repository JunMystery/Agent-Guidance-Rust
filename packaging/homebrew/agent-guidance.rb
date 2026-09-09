class AgentGuidance < Formula
  desc "Ultra-fast, zero-overhead Rust MCP server for AI agent steering"
  homepage "https://github.com/JunMystery/Agent-Guidance-Rust"
  version "1.5.5"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/JunMystery/Agent-Guidance-Rust/releases/download/v1.5.5/agent-guidance-macos-aarch64.tar.gz"
      sha256 :no_check
    else
      url "https://github.com/JunMystery/Agent-Guidance-Rust/releases/download/v1.5.5/agent-guidance-macos-x86_64.tar.gz"
      sha256 :no_check
    end
  end

  on_linux do
    url "https://github.com/JunMystery/Agent-Guidance-Rust/releases/download/v1.5.5/agent-guidance-linux-x86_64.tar.gz"
    sha256 :no_check
  end

  def install
    bin.install "agent-guidance"
  end

  def post_install
    system "#{bin}/agent-guidance", "--setup"
  end

  test do
    system "#{bin}/agent-guidance", "--verify-setup"
  end
end
