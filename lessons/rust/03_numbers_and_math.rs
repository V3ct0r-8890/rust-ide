// Lesson 3: Numbers and math

fn main() {
    // Integers: i8 i16 i32 i64 i128 isize (signed) and u8 u16 u32 u64 u128 usize (unsigned).
    // Floats: f32 and f64.
    let a = 17;
    let b = 5;
    println!("{} + {} = {}", a, b, a + b);
    println!("{} - {} = {}", a, b, a - b);
    println!("{} * {} = {}", a, b, a * b);
    println!("{} / {} = {} (whole numbers divide without a fraction)", a, b, a / b);
    println!("{} % {} = {} (the remainder)", a, b, a % b);

    // Floating point division keeps the fraction.
    let x = 17.0;
    let y = 5.0;
    println!("{x} / {y} = {}", x / y);

    // Rust never converts between number types silently. Use `as`.
    let whole: i32 = 7;
    let fraction = whole as f64 / 2.0;
    println!("{fraction}");
    let truncated = 3.99_f64 as i32; // cuts off the fraction
    println!("{truncated}");

    // Methods on numbers.
    println!("{}", 2_i32.pow(10)); // 1024
    println!("{}", (-8_i32).abs());
    println!("{:.3}", 2.0_f64.sqrt()); // {:.3} prints three decimals
    println!("{}", 10_i32.max(3));
    println!("{}", 7.5_f64.floor());

    // Overflow: a u8 cannot hold 256. Debug builds stop the program;
    // checked_add returns None instead, so you can handle it.
    let big: u8 = 250;
    println!("{:?}", big.checked_add(10));
    println!("{:?}", big.checked_add(5));
    println!("{}", big.wrapping_add(10)); // wraps around to 4
    println!("{}", big.saturating_add(10)); // stops at 255

    // Formatting numbers.
    println!("[{:5}]", 42); // width 5
    println!("[{:<5}]", 42); // left aligned
    println!("[{:05}]", 42); // padded with zeros
    println!("{:x} {:b} {:o}", 255, 5, 8); // hex, binary, octal

    // Try it: compute the area of a circle with radius 3.0 (use std::f64::consts::PI).
}
