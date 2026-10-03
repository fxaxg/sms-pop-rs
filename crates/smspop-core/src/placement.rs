//! 候选条该摆在光标的哪个位置。
//!
//! 抽成纯几何函数是刻意的：这是最容易写错、又完全不需要真实桌面就能测死的一块。
//! 真机上"位置偏了 / 点不到"这类问题，八成都能在这里用单测先拦住。

/// 屏幕上的一个矩形（DIP 单位，可直接喂给窗口）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectD {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl RectD {
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn right(&self) -> f64 {
        self.left + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.top + self.height
    }
}

/// 物理像素矩形。UIA 和 Win32 返回的都是物理像素，所以和 [`RectD`] 区分开，
/// 免得哪天把两种单位混着用还看不出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PixelRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_caret_point(x: i32, y: i32, height: i32) -> Self {
        Self::new(x, y, x, y + height.max(1))
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn is_empty(&self) -> bool {
        self.width() <= 0 && self.height() <= 0
    }

    /// 按 DPI 换算成 DIP。
    ///
    /// `dpi` 是目标显示器 DPI（96 = 100%）。传入 ≤ 0 或 96 都按 1.0 处理。
    pub fn to_dips(self, dpi: u32) -> RectD {
        let scale = dip_scale(dpi);

        RectD::new(
            self.left as f64 * scale,
            self.top as f64 * scale,
            self.width().max(1) as f64 * scale,
            self.height().max(1) as f64 * scale,
        )
    }
}

/// 物理像素 → DIP 的比例。96 DPI 正好是 1.0。
pub fn dip_scale(dpi: u32) -> f64 {
    if dpi == 0 {
        return 1.0;
    }

    96.0 / dpi as f64
}

/// 候选窗与光标之间的默认间距。
///
/// ★ 默认 24 而不是 6：中文输入法的候选窗也出现在光标正下方，
///   间距太小会和它**重叠被挡住**（实测被微信输入法挡过）。
///   24 大致能让开一行候选词；输入法候选栏更高就再调大。
pub const DEFAULT_GAP: f64 = 24.0;

/// 离工作区边缘至少留多少。
pub const DEFAULT_EDGE_PADDING: f64 = 8.0;

/// 把候选窗摆在光标**下方**；下方放不下就翻到**上方**；水平方向夹在工作区内。
///
/// 返回 `(left, top)`。
pub fn compute(
    caret: RectD,
    window_width: f64,
    window_height: f64,
    work_area: RectD,
    gap: f64,
    edge_padding: f64,
) -> (f64, f64) {
    let left = horizontal(caret.left, window_width, work_area, edge_padding);
    let top = vertical(caret, window_height, work_area, gap, edge_padding);

    (left, top)
}

/// 水平：优先与光标左边缘对齐，右边放不下就往左推。
fn horizontal(caret_left: f64, window_width: f64, work_area: RectD, edge_padding: f64) -> f64 {
    let min_left = work_area.left + edge_padding;
    let max_left = work_area.right() - edge_padding - window_width;

    // 工作区比窗口还窄（极端情况）—— 贴着左边放，别让 clamp 把 min/max 搞反
    if max_left < min_left {
        return min_left;
    }

    caret_left.clamp(min_left, max_left)
}

/// 垂直：默认在光标下方；下方放不下就翻到上方；再不行就贴着上边。
fn vertical(
    caret: RectD,
    window_height: f64,
    work_area: RectD,
    gap: f64,
    edge_padding: f64,
) -> f64 {
    let min_top = work_area.top + edge_padding;
    let max_top = work_area.bottom() - edge_padding - window_height;

    if max_top < min_top {
        return min_top;
    }

    let below = caret.bottom() + gap;

    if below <= max_top {
        return below;
    }

    // 下方放不下 → 试试光标上方
    let above = caret.top - gap - window_height;

    if above >= min_top {
        above
    } else {
        max_top
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: RectD = RectD {
        left: 0.0,
        top: 0.0,
        width: 1920.0,
        height: 1040.0,
    };

    fn caret(left: f64, top: f64) -> RectD {
        RectD::new(left, top, 1.0, 18.0)
    }

    #[test]
    fn 默认摆在光标下方_与左边缘对齐() {
        let (left, top) = compute(
            caret(300.0, 500.0),
            200.0,
            40.0,
            SCREEN,
            DEFAULT_GAP,
            DEFAULT_EDGE_PADDING,
        );

        assert_eq!(left, 300.0);
        // 500 + 18 + 24
        assert_eq!(top, 542.0);
    }

    #[test]
    fn 右边放不下时往左推() {
        // 光标在很靠右的位置，200 宽的窗口放不下
        let (left, _) = compute(
            caret(1900.0, 500.0),
            200.0,
            40.0,
            SCREEN,
            DEFAULT_GAP,
            DEFAULT_EDGE_PADDING,
        );

        // 1920 - 8 - 200
        assert_eq!(left, 1712.0);
    }

    #[test]
    fn 左边也不会溢出() {
        let (left, _) = compute(
            caret(-50.0, 500.0),
            200.0,
            40.0,
            SCREEN,
            DEFAULT_GAP,
            DEFAULT_EDGE_PADDING,
        );

        assert_eq!(left, 8.0);
    }

    #[test]
    fn 下方放不下就翻到光标上方() {
        // 光标贴近屏幕底部，下方只够 10px
        let (_, top) = compute(
            caret(300.0, 1020.0),
            200.0,
            40.0,
            SCREEN,
            DEFAULT_GAP,
            DEFAULT_EDGE_PADDING,
        );

        // 1020 - 24 - 40
        assert_eq!(top, 956.0);
    }

    #[test]
    fn 上下都放不下时贴着工作区底部() {
        // 窗口比工作区还高
        let (_, top) = compute(
            caret(300.0, 500.0),
            200.0,
            2000.0,
            SCREEN,
            DEFAULT_GAP,
            DEFAULT_EDGE_PADDING,
        );

        // max_top < min_top，直接贴顶
        assert_eq!(top, 8.0);
    }

    #[test]
    fn 工作区比窗口窄时贴左边且不崩溃() {
        let tiny = RectD::new(100.0, 100.0, 50.0, 50.0);

        let (left, _) = compute(caret(300.0, 300.0), 200.0, 40.0, tiny, DEFAULT_GAP, 8.0);

        assert_eq!(left, 108.0);
    }

    #[test]
    fn 第二显示器的工作区也正确() {
        // 副屏在主屏右边：x 从 1920 开始
        let second = RectD::new(1920.0, 0.0, 1920.0, 1040.0);

        let (left, top) = compute(caret(2000.0, 500.0), 200.0, 40.0, second, 24.0, 8.0);

        assert_eq!(left, 2000.0);
        assert_eq!(top, 542.0);
    }

    #[test]
    fn dpi_换算() {
        assert_eq!(dip_scale(96), 1.0);
        assert_eq!(dip_scale(120), 0.8); // 125%
        assert_eq!(dip_scale(144), 96.0 / 144.0); // 150%
        assert_eq!(dip_scale(192), 0.5); // 200%
                                         // 拿不到 DPI 时按 1.0，别把窗口算到屏幕外
        assert_eq!(dip_scale(0), 1.0);
    }

    #[test]
    fn 物理像素转_dip() {
        let rect = PixelRect::new(240, 480, 240, 516);

        let dips = rect.to_dips(120); // 125%

        assert_eq!(dips.left, 192.0);
        assert_eq!(dips.top, 384.0);
        // 零宽的光标也要有最小宽度，否则后续按宽/高计算会得到 0
        assert!(dips.width >= 0.8);
        assert!((dips.height - 28.8).abs() < 1e-9);
    }

    #[test]
    fn 从光标点构造的矩形高度至少为_1() {
        let rect = PixelRect::from_caret_point(100, 200, 0);

        assert_eq!(rect.height(), 1);
        assert_eq!(rect.width(), 0);
        assert!(!rect.is_empty());
    }
}
