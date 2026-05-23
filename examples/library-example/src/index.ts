import { Circle, Point, is_inside, is_inside_move } from "./math.rs";

let circle = new Circle(10, new Point(0, 0));

console.log(is_inside(circle, circle.center));
