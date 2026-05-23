import { Circle, Point, is_inside, is_inside_move } from "./math.rs";

let circle = new Circle(10, new Point(0, 0));
let center = circle.center;
console.log("circle.center", circle.center.x, circle.center.y);
console.log("circle", center.x, center.y);
center.x = 1;
center.y = 1;
console.log("circle.center (upd)", circle.center.x, circle.center.y);
console.log("circle (upd)", center.x, center.y);

circle.center = center;

console.log("reassigned");

console.log("circle.center", circle.center.x, circle.center.y);
console.log("circle", center.x, center.y);
center.x = 2;
center.y = 2;
console.log("circle.center (upd)", circle.center.x, circle.center.y);
console.log("circle (upd)", center.x, center.y);
