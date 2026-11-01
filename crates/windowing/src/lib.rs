#![no_std]

pub const MAX_WINDOWS: usize = 4;
pub const MAX_WIDGETS: usize = 8;
pub const MAX_DAMAGE: usize = 16;
pub const MAX_TEXT: usize = 32;

pub const COLOR_DESKTOP: u32 = 0x0008_101d;
pub const COLOR_PANEL: u32 = 0x000d_1726;
pub const COLOR_SURFACE: u32 = 0x0013_2032;
pub const COLOR_SURFACE_RAISED: u32 = 0x0018_283c;
pub const COLOR_BORDER: u32 = 0x0033_4a62;
pub const COLOR_TEXT: u32 = 0x00e6_edf7;
pub const COLOR_MUTED: u32 = 0x008d_a2ba;
pub const COLOR_ACCENT: u32 = 0x000d_7ff2;
pub const COLOR_HEALTHY: u32 = 0x002f_d49a;
pub const COLOR_DARK_TEXT: u32 = 0x0008_101d;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn contains(self, x: i16, y: i16) -> bool {
        x >= self.x
            && y >= self.y
            && i32::from(x) < i32::from(self.x) + i32::from(self.width)
            && i32::from(y) < i32::from(self.y) + i32::from(self.height)
    }

    fn intersect(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (i32::from(self.x) + i32::from(self.width))
            .min(i32::from(other.x) + i32::from(other.width));
        let bottom = (i32::from(self.y) + i32::from(self.height))
            .min(i32::from(other.y) + i32::from(other.height));
        (right > i32::from(left) && bottom > i32::from(top)).then_some(Self {
            x: left,
            y: top,
            width: (right - i32::from(left)) as u16,
            height: (bottom - i32::from(top)) as u16,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bounds,
    Capacity,
    NotFound,
    Stale,
    State,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowHandle {
    slot: u8,
    generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidgetKind {
    Label,
    Button,
    TextField,
    Menu,
    Table,
}

#[derive(Clone, Copy)]
struct Widget {
    kind: WidgetKind,
    rect: Rect,
    text: [u8; MAX_TEXT],
    len: u8,
    cursor: u8,
    anchor: u8,
    submitted: bool,
}

impl Widget {
    const fn empty() -> Self {
        Self {
            kind: WidgetKind::Label,
            rect: Rect::new(0, 0, 0, 0),
            text: [0; MAX_TEXT],
            len: 0,
            cursor: 0,
            anchor: 0,
            submitted: false,
        }
    }
}

#[derive(Clone, Copy)]
struct Window {
    generation: u32,
    rect: Rect,
    restore: Rect,
    state: WindowState,
    title: [u8; 16],
    title_len: u8,
    z: u8,
    focused: bool,
    trusted: bool,
    frame_pending: bool,
    widgets: [Widget; MAX_WIDGETS],
    widget_count: u8,
}

impl Window {
    const fn empty() -> Self {
        Self {
            generation: 0,
            rect: Rect::new(0, 0, 0, 0),
            restore: Rect::new(0, 0, 0, 0),
            state: WindowState::Closed,
            title: [0; 16],
            title_len: 0,
            z: 0,
            focused: false,
            trusted: false,
            frame_pending: false,
            widgets: [Widget::empty(); MAX_WIDGETS],
            widget_count: 0,
        }
    }
}

pub struct Desktop {
    width: u16,
    height: u16,
    windows: [Window; MAX_WINDOWS],
    generations: [u32; MAX_WINDOWS],
    damage: [Rect; MAX_DAMAGE],
    damage_count: u8,
    focused: Option<WindowHandle>,
    frame_callbacks: u8,
}

impl Desktop {
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            windows: [Window::empty(); MAX_WINDOWS],
            generations: [0; MAX_WINDOWS],
            damage: [Rect::new(0, 0, 0, 0); MAX_DAMAGE],
            damage_count: 0,
            focused: None,
            frame_callbacks: 0,
        }
    }

    fn screen(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    fn window(&self, handle: WindowHandle) -> Result<&Window, Error> {
        self.windows
            .get(handle.slot as usize)
            .filter(|window| window.generation == handle.generation && handle.generation != 0)
            .ok_or(Error::Stale)
    }

    fn window_mut(&mut self, handle: WindowHandle) -> Result<&mut Window, Error> {
        self.windows
            .get_mut(handle.slot as usize)
            .filter(|window| window.generation == handle.generation && handle.generation != 0)
            .ok_or(Error::Stale)
    }

    fn damage(&mut self, rect: Rect) {
        let Some(rect) = rect.intersect(self.screen()) else {
            return;
        };
        if usize::from(self.damage_count) < MAX_DAMAGE {
            self.damage[usize::from(self.damage_count)] = rect;
            self.damage_count += 1;
        } else {
            self.damage[0] = self.screen();
            self.damage_count = 1;
        }
    }

    pub fn create_window(
        &mut self,
        rect: Rect,
        title: &[u8],
        trusted: bool,
    ) -> Result<WindowHandle, Error> {
        if rect.width < 20 || rect.height < 16 || rect.intersect(self.screen()) != Some(rect) {
            return Err(Error::Bounds);
        }
        let slot = self
            .windows
            .iter()
            .position(|window| window.generation == 0)
            .ok_or(Error::Capacity)?;
        let generation = self.generations[slot]
            .checked_add(1)
            .ok_or(Error::Capacity)?;
        self.generations[slot] = generation;
        let mut window = Window::empty();
        window.generation = generation;
        window.rect = rect;
        window.restore = rect;
        window.state = WindowState::Normal;
        window.trusted = trusted;
        window.z = self
            .windows
            .iter()
            .map(|window| window.z)
            .max()
            .unwrap_or(0)
            + 1;
        window.title_len = title.len().min(window.title.len()) as u8;
        window.title[..usize::from(window.title_len)]
            .copy_from_slice(&title[..usize::from(window.title_len)]);
        self.windows[slot] = window;
        let handle = WindowHandle {
            slot: slot as u8,
            generation,
        };
        self.focus(handle)?;
        self.damage(rect);
        Ok(handle)
    }

    pub fn focus(&mut self, handle: WindowHandle) -> Result<(), Error> {
        if self.window(handle)?.state == WindowState::Closed {
            return Err(Error::State);
        }
        let next_z = self
            .windows
            .iter()
            .map(|window| window.z)
            .max()
            .unwrap_or(0)
            + 1;
        for window in &mut self.windows {
            window.focused = false;
        }
        let rect = {
            let window = self.window_mut(handle)?;
            window.focused = true;
            window.z = next_z;
            window.rect
        };
        self.focused = Some(handle);
        self.damage(rect);
        Ok(())
    }

    pub fn move_window(&mut self, handle: WindowHandle, x: i16, y: i16) -> Result<(), Error> {
        let old = self.window(handle)?.rect;
        let next = Rect::new(x, y, old.width, old.height);
        if next.intersect(self.screen()) != Some(next) {
            return Err(Error::Bounds);
        }
        let window = self.window_mut(handle)?;
        if window.state != WindowState::Normal {
            return Err(Error::State);
        }
        window.rect = next;
        window.restore = next;
        self.damage(old);
        self.damage(next);
        Ok(())
    }

    pub fn resize_window(
        &mut self,
        handle: WindowHandle,
        width: u16,
        height: u16,
    ) -> Result<(), Error> {
        let old = self.window(handle)?.rect;
        let next = Rect::new(old.x, old.y, width, height);
        if width < 20 || height < 16 || next.intersect(self.screen()) != Some(next) {
            return Err(Error::Bounds);
        }
        let window = self.window_mut(handle)?;
        if window.state != WindowState::Normal {
            return Err(Error::State);
        }
        window.rect = next;
        window.restore = next;
        self.damage(old);
        self.damage(next);
        Ok(())
    }

    pub fn minimize(&mut self, handle: WindowHandle) -> Result<(), Error> {
        let rect = self.window(handle)?.rect;
        self.window_mut(handle)?.state = WindowState::Minimized;
        if self.focused == Some(handle) {
            self.focused = None;
        }
        self.damage(rect);
        Ok(())
    }

    pub fn maximize(&mut self, handle: WindowHandle) -> Result<(), Error> {
        let screen = self.screen();
        let old = self.window(handle)?.rect;
        let window = self.window_mut(handle)?;
        if window.state == WindowState::Normal {
            window.restore = window.rect;
        }
        window.rect = Rect::new(0, 8, screen.width, screen.height.saturating_sub(8));
        window.state = WindowState::Maximized;
        self.damage(old);
        self.damage(screen);
        Ok(())
    }

    pub fn restore(&mut self, handle: WindowHandle) -> Result<(), Error> {
        let old = self.window(handle)?.rect;
        let restore = self.window(handle)?.restore;
        let window = self.window_mut(handle)?;
        window.rect = restore;
        window.state = WindowState::Normal;
        self.damage(old);
        self.damage(restore);
        Ok(())
    }

    pub fn close(&mut self, handle: WindowHandle) -> Result<(), Error> {
        let rect = self.window(handle)?.rect;
        *self.window_mut(handle)? = Window::empty();
        if self.focused == Some(handle) {
            self.focused = None;
        }
        self.damage(rect);
        Ok(())
    }

    pub fn add_widget(
        &mut self,
        handle: WindowHandle,
        kind: WidgetKind,
        rect: Rect,
        text: &[u8],
    ) -> Result<u8, Error> {
        let window = self.window_mut(handle)?;
        let index = usize::from(window.widget_count);
        if index == MAX_WIDGETS {
            return Err(Error::Capacity);
        }
        if rect.width == 0 || rect.height == 0 {
            return Err(Error::Bounds);
        }
        let mut widget = Widget::empty();
        widget.kind = kind;
        widget.rect = rect;
        widget.len = text.len().min(MAX_TEXT) as u8;
        widget.cursor = widget.len;
        widget.anchor = widget.len;
        widget.text[..usize::from(widget.len)].copy_from_slice(&text[..usize::from(widget.len)]);
        window.widgets[index] = widget;
        window.widget_count += 1;
        let damage = Rect::new(
            window.rect.x + rect.x,
            window.rect.y + 10 + rect.y,
            rect.width,
            rect.height,
        );
        self.damage(damage);
        Ok(index as u8)
    }

    fn text_field_mut(&mut self, handle: WindowHandle, widget: u8) -> Result<&mut Widget, Error> {
        let window = self.window_mut(handle)?;
        let widget = window
            .widgets
            .get_mut(usize::from(widget))
            .filter(|_| widget < window.widget_count)
            .ok_or(Error::NotFound)?;
        (widget.kind == WidgetKind::TextField)
            .then_some(widget)
            .ok_or(Error::State)
    }

    pub fn text_insert(&mut self, handle: WindowHandle, widget: u8, byte: u8) -> Result<(), Error> {
        let field = self.text_field_mut(handle, widget)?;
        let start = field.cursor.min(field.anchor) as usize;
        let end = field.cursor.max(field.anchor) as usize;
        let retained = usize::from(field.len) - end;
        let next_len = start + 1 + retained;
        if next_len > MAX_TEXT {
            return Err(Error::Capacity);
        }
        field
            .text
            .copy_within(end..usize::from(field.len), start + 1);
        field.text[start] = byte;
        field.len = next_len as u8;
        field.cursor = (start + 1) as u8;
        field.anchor = field.cursor;
        Ok(())
    }

    pub fn text_left(
        &mut self,
        handle: WindowHandle,
        widget: u8,
        selecting: bool,
    ) -> Result<(), Error> {
        let field = self.text_field_mut(handle, widget)?;
        field.cursor = field.cursor.saturating_sub(1);
        if !selecting {
            field.anchor = field.cursor;
        }
        Ok(())
    }

    pub fn text_right(
        &mut self,
        handle: WindowHandle,
        widget: u8,
        selecting: bool,
    ) -> Result<(), Error> {
        let field = self.text_field_mut(handle, widget)?;
        field.cursor = (field.cursor + 1).min(field.len);
        if !selecting {
            field.anchor = field.cursor;
        }
        Ok(())
    }

    pub fn text_backspace(&mut self, handle: WindowHandle, widget: u8) -> Result<(), Error> {
        let field = self.text_field_mut(handle, widget)?;
        let mut start = field.cursor.min(field.anchor) as usize;
        let end = field.cursor.max(field.anchor) as usize;
        let remove_end = if start == end {
            if start == 0 {
                return Ok(());
            }
            start -= 1;
            end
        } else {
            end
        };
        field
            .text
            .copy_within(remove_end..usize::from(field.len), start);
        field.len -= (remove_end - start) as u8;
        field.cursor = start as u8;
        field.anchor = field.cursor;
        Ok(())
    }

    pub fn text_submit(&mut self, handle: WindowHandle, widget: u8) -> Result<(), Error> {
        self.text_field_mut(handle, widget)?.submitted = true;
        Ok(())
    }

    pub fn text_value(&self, handle: WindowHandle, widget: u8) -> Result<(&[u8], bool), Error> {
        let window = self.window(handle)?;
        let field = window
            .widgets
            .get(usize::from(widget))
            .filter(|_| widget < window.widget_count)
            .ok_or(Error::NotFound)?;
        if field.kind != WidgetKind::TextField {
            return Err(Error::State);
        }
        Ok((&field.text[..usize::from(field.len)], field.submitted))
    }

    pub fn pointer_down(&mut self, x: i16, y: i16) -> Result<Option<WindowHandle>, Error> {
        let target = self
            .windows
            .iter()
            .enumerate()
            .filter(|(_, window)| {
                window.generation != 0
                    && window.state != WindowState::Minimized
                    && window.rect.contains(x, y)
            })
            .max_by_key(|(_, window)| window.z)
            .map(|(slot, window)| WindowHandle {
                slot: slot as u8,
                generation: window.generation,
            });
        if let Some(handle) = target {
            self.focus(handle)?;
        }
        Ok(target)
    }

    pub fn request_frame(&mut self, handle: WindowHandle) -> Result<(), Error> {
        if self.frame_callbacks as usize == MAX_WINDOWS {
            return Err(Error::Capacity);
        }
        let window = self.window_mut(handle)?;
        if window.frame_pending {
            return Err(Error::State);
        }
        window.frame_pending = true;
        self.frame_callbacks += 1;
        Ok(())
    }

    pub fn complete_frames(&mut self) -> u8 {
        let completed = self.frame_callbacks;
        for window in &mut self.windows {
            window.frame_pending = false;
        }
        self.frame_callbacks = 0;
        completed
    }

    pub fn damage_count(&self) -> u8 {
        self.damage_count
    }
    pub fn clear_damage(&mut self) {
        self.damage_count = 0;
    }
    pub fn state(&self, handle: WindowHandle) -> Result<WindowState, Error> {
        Ok(self.window(handle)?.state)
    }
    pub fn focused(&self) -> Option<WindowHandle> {
        self.focused
    }

    pub fn render(&self, pixels: &mut [u32], stride: usize) -> Result<(), Error> {
        if stride < usize::from(self.width) || pixels.len() < stride * usize::from(self.height) {
            return Err(Error::Bounds);
        }
        fill(pixels, stride, self.screen(), COLOR_DESKTOP, self.screen());
        fill(
            pixels,
            stride,
            Rect::new(0, 0, self.width, 8),
            COLOR_PANEL,
            self.screen(),
        );
        for z in 1..=u8::MAX {
            for window in &self.windows {
                if window.generation == 0
                    || window.z != z
                    || matches!(window.state, WindowState::Minimized | WindowState::Closed)
                {
                    continue;
                }
                let clip = window.rect.intersect(self.screen()).ok_or(Error::Bounds)?;
                fill(
                    pixels,
                    stride,
                    Rect::new(
                        window.rect.x + 1,
                        window.rect.y + 1,
                        window.rect.width,
                        window.rect.height,
                    ),
                    COLOR_DESKTOP,
                    self.screen(),
                );
                fill(pixels, stride, window.rect, COLOR_SURFACE, clip);
                border(pixels, stride, window.rect, COLOR_BORDER, clip);
                fill(
                    pixels,
                    stride,
                    Rect::new(window.rect.x, window.rect.y, window.rect.width, 9),
                    if window.focused {
                        COLOR_SURFACE_RAISED
                    } else {
                        COLOR_PANEL
                    },
                    clip,
                );
                draw_text(
                    pixels,
                    stride,
                    window.rect.x + 2,
                    window.rect.y + 1,
                    &window.title[..usize::from(window.title_len)],
                    COLOR_TEXT,
                    clip,
                );
                if window.trusted {
                    fill(
                        pixels,
                        stride,
                        Rect::new(
                            window.rect.x + window.rect.width as i16 - 5,
                            window.rect.y + 2,
                            3,
                            3,
                        ),
                        if window.focused {
                            COLOR_HEALTHY
                        } else {
                            COLOR_MUTED
                        },
                        clip,
                    );
                }
                for widget in &window.widgets[..usize::from(window.widget_count)] {
                    let rect = Rect::new(
                        window.rect.x + widget.rect.x,
                        window.rect.y + 10 + widget.rect.y,
                        widget.rect.width,
                        widget.rect.height,
                    );
                    match widget.kind {
                        WidgetKind::Label => draw_text(
                            pixels,
                            stride,
                            rect.x,
                            rect.y,
                            &widget.text[..usize::from(widget.len)],
                            COLOR_TEXT,
                            clip,
                        ),
                        WidgetKind::Button | WidgetKind::Menu => {
                            fill(pixels, stride, rect, COLOR_ACCENT, clip);
                            draw_text(
                                pixels,
                                stride,
                                rect.x + 2,
                                rect.y + 2,
                                &widget.text[..usize::from(widget.len)],
                                COLOR_TEXT,
                                clip,
                            );
                        }
                        WidgetKind::TextField => {
                            fill(pixels, stride, rect, COLOR_PANEL, clip);
                            border(pixels, stride, rect, COLOR_BORDER, clip);
                            draw_text(
                                pixels,
                                stride,
                                rect.x + 1,
                                rect.y + 2,
                                &widget.text[..usize::from(widget.len)],
                                COLOR_TEXT,
                                clip,
                            );
                        }
                        WidgetKind::Table => {
                            fill(pixels, stride, rect, COLOR_PANEL, clip);
                            border(pixels, stride, rect, COLOR_BORDER, clip);
                            draw_text(
                                pixels,
                                stride,
                                rect.x + 1,
                                rect.y + 1,
                                &widget.text[..usize::from(widget.len)],
                                COLOR_TEXT,
                                clip,
                            );
                            if rect.height > 7 {
                                fill(
                                    pixels,
                                    stride,
                                    Rect::new(rect.x, rect.y + 7, rect.width, 1),
                                    COLOR_BORDER,
                                    clip,
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn fill(pixels: &mut [u32], stride: usize, rect: Rect, color: u32, clip: Rect) {
    let Some(rect) = rect.intersect(clip) else {
        return;
    };
    for y in rect.y..rect.y + rect.height as i16 {
        for x in rect.x..rect.x + rect.width as i16 {
            pixels[y as usize * stride + x as usize] = color;
        }
    }
}

fn border(pixels: &mut [u32], stride: usize, rect: Rect, color: u32, clip: Rect) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    fill(
        pixels,
        stride,
        Rect::new(rect.x, rect.y, rect.width, 1),
        color,
        clip,
    );
    fill(
        pixels,
        stride,
        Rect::new(rect.x, rect.y + rect.height as i16 - 1, rect.width, 1),
        color,
        clip,
    );
    fill(
        pixels,
        stride,
        Rect::new(rect.x, rect.y, 1, rect.height),
        color,
        clip,
    );
    fill(
        pixels,
        stride,
        Rect::new(rect.x + rect.width as i16 - 1, rect.y, 1, rect.height),
        color,
        clip,
    );
}

pub fn paint_rect(pixels: &mut [u32], stride: usize, rect: Rect, color: u32) -> Result<(), Error> {
    if stride == 0 || !pixels.len().is_multiple_of(stride) {
        return Err(Error::Bounds);
    }
    let width = u16::try_from(stride).map_err(|_| Error::Bounds)?;
    let height = u16::try_from(pixels.len() / stride).map_err(|_| Error::Bounds)?;
    let clip = Rect::new(0, 0, width, height);
    fill(pixels, stride, rect, color, clip);
    Ok(())
}

pub fn paint_border(
    pixels: &mut [u32],
    stride: usize,
    rect: Rect,
    color: u32,
) -> Result<(), Error> {
    if stride == 0 || !pixels.len().is_multiple_of(stride) {
        return Err(Error::Bounds);
    }
    let width = u16::try_from(stride).map_err(|_| Error::Bounds)?;
    let height = u16::try_from(pixels.len() / stride).map_err(|_| Error::Bounds)?;
    let clip = Rect::new(0, 0, width, height);
    border(pixels, stride, rect, color, clip);
    Ok(())
}

pub fn paint_text(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    text: &[u8],
    color: u32,
) -> Result<(), Error> {
    paint_text_scaled(pixels, stride, x, y, text, color, 1)
}

pub fn paint_text_scaled(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    text: &[u8],
    color: u32,
    scale: u16,
) -> Result<(), Error> {
    if stride == 0 || !pixels.len().is_multiple_of(stride) {
        return Err(Error::Bounds);
    }
    if scale == 0 || scale > 4 {
        return Err(Error::Bounds);
    }
    let width = u16::try_from(stride).map_err(|_| Error::Bounds)?;
    let height = u16::try_from(pixels.len() / stride).map_err(|_| Error::Bounds)?;
    let clip = Rect::new(0, 0, width, height);
    let scale = scale as i16;
    for (index, byte) in text.iter().enumerate() {
        let ox = x + index as i16 * 4 * scale;
        for row in 0..5_i16 {
            let bits = glyph(*byte)[row as usize];
            for column in 0..3_i16 {
                if bits & (1 << (2 - column)) != 0 {
                    fill(
                        pixels,
                        stride,
                        Rect::new(
                            ox + column * scale,
                            y + row * scale,
                            scale as u16,
                            scale as u16,
                        ),
                        color,
                        clip,
                    );
                }
            }
        }
    }
    Ok(())
}

fn draw_text(
    pixels: &mut [u32],
    stride: usize,
    x: i16,
    y: i16,
    text: &[u8],
    color: u32,
    clip: Rect,
) {
    for (index, byte) in text.iter().enumerate() {
        let ox = x + (index as i16 * 4);
        for row in 0..5_i16 {
            let bits = glyph(*byte)[row as usize];
            for column in 0..3_i16 {
                if bits & (1 << (2 - column)) != 0 {
                    fill(
                        pixels,
                        stride,
                        Rect::new(ox + column, y + row, 1, 1),
                        color,
                        clip,
                    );
                }
            }
        }
    }
}

fn glyph(byte: u8) -> [u8; 5] {
    match byte.to_ascii_uppercase() {
        b'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        b'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        b'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        b'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        b'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        b'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        b'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        b'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        b'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        b'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        b'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        b'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        b'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        b'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        b'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        b'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        b'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        b'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        b'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        b'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        b'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        b'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        b'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        b'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        b'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        b'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        b'0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        b'1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        b'2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        b'3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        b'4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        b'5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        b'6' => [0b011, 0b100, 0b111, 0b101, 0b111],
        b'7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        b'8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        b'9' => [0b111, 0b101, 0b111, 0b001, 0b110],
        b'-' => [0, 0, 0b111, 0, 0],
        b'_' => [0, 0, 0, 0, 0b111],
        b'.' => [0, 0, 0, 0, 0b010],
        b':' => [0, 0b010, 0, 0b010, 0],
        b'|' => [0b010, 0b010, 0b010, 0b010, 0b010],
        b'%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        b'/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        b'>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        b' ' => [0; 5],
        _ => [0b111, 0b001, 0b010, 0, 0b010],
    }
}

pub fn pixel_hash(pixels: &[u32]) -> u64 {
    pixels.iter().fold(0xcbf2_9ce4_8422_2325, |hash, pixel| {
        (hash ^ u64::from(*pixel)).wrapping_mul(0x100_0000_01b3)
    })
}

pub fn contract_probe() -> bool {
    let mut desktop = Desktop::new(32, 24);
    let Ok(controller) = desktop.create_window(Rect::new(1, 8, 25, 16), b"Control", true) else {
        return false;
    };
    let Ok(domain) = desktop.create_window(Rect::new(8, 8, 23, 16), b"Domain", false) else {
        return false;
    };
    let Ok(field) = desktop.add_widget(
        controller,
        WidgetKind::TextField,
        Rect::new(2, 2, 20, 7),
        b"ab",
    ) else {
        return false;
    };
    if desktop
        .add_widget(
            controller,
            WidgetKind::Button,
            Rect::new(2, 11, 12, 6),
            b"Run",
        )
        .is_err()
        || desktop
            .add_widget(domain, WidgetKind::Menu, Rect::new(1, 1, 14, 6), b"File")
            .is_err()
        || desktop.text_left(controller, field, false).is_err()
        || desktop.text_insert(controller, field, b'X').is_err()
        || desktop.text_right(controller, field, true).is_err()
        || desktop.text_backspace(controller, field).is_err()
        || desktop.text_submit(controller, field).is_err()
        || desktop.move_window(domain, 7, 7).is_err()
        || desktop.resize_window(domain, 24, 17).is_err()
        || desktop.minimize(domain).is_err()
        || desktop.restore(domain).is_err()
        || desktop.maximize(domain).is_err()
        || desktop.restore(domain).is_err()
        || desktop.focus(controller).is_err()
        || desktop.request_frame(controller).is_err()
        || desktop.complete_frames() != 1
    {
        return false;
    }
    let mut pixels = [0_u32; 32 * 24];
    desktop.render(&mut pixels, 32).is_ok()
        && pixel_hash(&pixels) != 0
        && desktop.damage_count() > 0
        && desktop.focused() == Some(controller)
        && desktop.close(domain).is_ok()
        && desktop.state(domain) == Err(Error::Stale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> (Desktop, WindowHandle, WindowHandle) {
        let mut desktop = Desktop::new(96, 64);
        let controller = desktop
            .create_window(Rect::new(3, 10, 52, 44), b"Ginux", true)
            .unwrap();
        let domain = desktop
            .create_window(Rect::new(38, 18, 54, 40), b"Rocky", false)
            .unwrap();
        desktop
            .add_widget(
                controller,
                WidgetKind::Label,
                Rect::new(3, 3, 30, 6),
                b"Domains",
            )
            .unwrap();
        desktop
            .add_widget(
                controller,
                WidgetKind::Menu,
                Rect::new(3, 11, 28, 7),
                b"Actions",
            )
            .unwrap();
        desktop
            .add_widget(
                controller,
                WidgetKind::Button,
                Rect::new(3, 21, 20, 7),
                b"Start",
            )
            .unwrap();
        desktop
            .add_widget(
                controller,
                WidgetKind::TextField,
                Rect::new(3, 31, 38, 7),
                b"rocky",
            )
            .unwrap();
        desktop
            .add_widget(
                domain,
                WidgetKind::Table,
                Rect::new(2, 3, 46, 25),
                b"PID NAME",
            )
            .unwrap();
        (desktop, controller, domain)
    }

    #[test]
    fn window_lifecycle_z_order_damage_and_frames_are_bounded() {
        let (mut desktop, controller, domain) = demo();
        assert_eq!(desktop.focused(), Some(domain));
        assert_eq!(desktop.pointer_down(4, 11), Ok(Some(controller)));
        assert_eq!(desktop.focused(), Some(controller));
        desktop.move_window(domain, 37, 17).unwrap();
        desktop.resize_window(domain, 55, 41).unwrap();
        desktop.minimize(domain).unwrap();
        assert_eq!(desktop.state(domain), Ok(WindowState::Minimized));
        desktop.restore(domain).unwrap();
        desktop.maximize(domain).unwrap();
        assert_eq!(desktop.state(domain), Ok(WindowState::Maximized));
        desktop.restore(domain).unwrap();
        desktop.request_frame(controller).unwrap();
        assert_eq!(desktop.request_frame(controller), Err(Error::State));
        assert_eq!(desktop.complete_frames(), 1);
        assert!(desktop.damage_count() <= MAX_DAMAGE as u8);
        desktop.clear_damage();
        assert_eq!(desktop.damage_count(), 0);
        desktop.close(domain).unwrap();
        assert_eq!(desktop.focus(domain), Err(Error::Stale));
    }

    #[test]
    fn text_field_supports_edit_selection_backspace_and_submit() {
        let (mut desktop, controller, _) = demo();
        let field = 3;
        desktop.text_left(controller, field, false).unwrap();
        desktop.text_left(controller, field, true).unwrap();
        desktop.text_insert(controller, field, b'X').unwrap();
        desktop.text_right(controller, field, false).unwrap();
        desktop.text_backspace(controller, field).unwrap();
        desktop.text_submit(controller, field).unwrap();
        assert_eq!(
            desktop.text_value(controller, field),
            Ok((&b"rocX"[..], true))
        );
    }

    #[test]
    fn deterministic_screenshot_matches_exact_fixture() {
        let (desktop, _, _) = demo();
        let mut first = [0_u32; 96 * 64];
        let mut second = [0_u32; 96 * 64];
        desktop.render(&mut first, 96).unwrap();
        desktop.render(&mut second, 96).unwrap();
        assert_eq!(first, second);
        assert_eq!(pixel_hash(&first), 0x828c_bd51_615c_3e2c);
    }

    #[test]
    fn contract_is_exercised_without_allocation() {
        assert!(contract_probe());
    }
}
