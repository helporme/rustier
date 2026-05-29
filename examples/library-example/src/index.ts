import { Circle, Point } from "./math.rs";

const point = new Point(5, 3);
const circle = new Circle(10, { x: 0, y: 0 });

console.log({
  "point in circle": circle.contains(point),
  distance: circle.center.distance(point),
});
