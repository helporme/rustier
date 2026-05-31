import { Point, Line } from "./math.rs";

const line = Line.of({ start: { x: 0, y: 0 }, end: { x: 1, y: 1 } });
line.start = Point.of({
  x: 10,
  y: 10,
});

line.start.x = 10;
console.log(line.start.x);
