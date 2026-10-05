//! 一些和 WinRT 打交道的零碎工具。

use smspop_core::uuid::Uuid;
use windows::core::GUID;
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};

/// `windows` 0.62 的 `IAsyncOperation` 是 `.await` 风格，
/// 但我们这些调用点都在同步代码里（而且大多在独立线程上），
/// 所以用 `pollster` 直接把它跑完。
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    pollster::block_on(future)
}

/// 限时驱动异步操作。超时会丢弃等待中的 future，让监督线程恢复控制权。
/// 这不保证底层 WinRT 操作已经取消，因此超时后调用方必须结束当前会话，不能继续复用。
pub fn block_on_timeout<F: std::future::Future>(
    future: F,
    timeout: std::time::Duration,
) -> crate::Result<F::Output> {
    struct WakeThread(std::thread::Thread);
    impl std::task::Wake for WakeThread {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(WakeThread(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let std::task::Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return Ok(result);
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err("蓝牙异步操作超时，结束本轮会话".into());
        }
        std::thread::park_timeout(remaining);
    }
}

/// 我们的 UUID（RFC 4122 顺序）→ Windows 的 GUID。
pub fn guid(uuid: Uuid) -> GUID {
    GUID::from_u128(uuid.to_u128())
}

/// 字节 → WinRT 的 `IBuffer`。
pub fn to_buffer(bytes: &[u8]) -> windows::core::Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(bytes)?;
    writer.DetachBuffer()
}

/// WinRT 的 `IBuffer` → 字节。
pub fn buffer_to_vec(buffer: &IBuffer) -> windows::core::Result<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let mut bytes = vec![0u8; buffer.Length()? as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use smspop_core::ancs::ANCS_SERVICE_UUID;

    #[test]
    fn 异步等待确实有截止时间() {
        let started = std::time::Instant::now();
        assert!(block_on_timeout(
            std::future::pending::<()>(),
            std::time::Duration::from_millis(20)
        )
        .is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn 异步完成直接返回结果() {
        assert_eq!(
            block_on_timeout(std::future::ready(42), std::time::Duration::from_secs(1)).unwrap(),
            42
        );
    }

    #[test]
    // 故意按 UUID 的 8-4-4-4-12 分组，好和文档里的写法对上
    #[allow(clippy::unusual_byte_groupings)]
    fn uuid_转_guid_的数值正确() {
        let converted = guid(ANCS_SERVICE_UUID);
        assert_eq!(
            converted,
            GUID::from_u128(0x7905_F431_B5CE_4E99_A40F_4B1E122D00D0)
        );
    }

    #[test]
    fn 字节进缓冲区再出来还是原样() {
        let original = [0xD0u8, 0x00, 0x2D, 0x12];
        let buffer = to_buffer(&original).unwrap();
        let round_tripped = buffer_to_vec(&buffer).unwrap();

        assert_eq!(round_tripped, original);
    }
}
