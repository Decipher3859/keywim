mod autostart;
mod compositor;

fn main() {
    println!("Keywim starting... ");
    compositor::initialize();
}
