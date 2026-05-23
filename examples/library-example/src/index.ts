import { Circle, Point, is_inside, is_inside_move } from "./math.rs";

let circle = Circle.of({
  radius: 1,
  center: {
    x: 0,
    y: 1,
  },
});

console.log(
  is_inside_move(circle, {
    x: 0,
    y: 1,
  }),
  is_inside_move(circle, {
    x: 1,
    y: 1,
  }),
  is_inside_move(circle, {
    x: 1,
    y: 2,
  }),
);
