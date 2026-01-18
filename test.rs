fn main() {
    let name = "FuckVim";
    println!("Hello, {}!", name);
    
    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();
    
    println!("Sum of {:?} is {}", numbers, sum);
    
    // Check syntax highlighting
    // ;;rust is awesome

}

struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
