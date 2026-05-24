import { Circle, Point } from "./math.rs";

const a = new Point(0, 0);
const b = Point.of(a);

a.translate(10, 10);
a.scale(2);

const c = Circle.of({ radius: 3, center: a });

console.log(a.distance(b), c.center.toString());
