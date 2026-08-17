class AgentGuidance < Formula
  desc "Token-optimized Agent Guidance MCP Rust server for AI coding tools"
  homepage "https://github.com/JunMystery/Agent-Guidance-Rust"
  version "1.4.4"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/JunMystery/Agent-Guidance-Rust/releases/download/v1.4.4/agent-guidance-macos-aarch64.tar.gz"
      sha256 "PLACEHOLDER_MAC_ARM_SHA256"
    else
      url "https://github.com/JunMystery/Agent-Guidance-Rust/releases/download/v1.4.4/agent-guidance-macos-x86_64.tar.gz"
      sha256 "PLACEHOLDER_MAC_INTEL_SHA256"
    end
  elsif OS.linux?
    url "https://github.com/JunMystery/Agent-Guidance-Rust/releases/download/v1.4.4/agent-guidance-linux-x86_64.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
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
