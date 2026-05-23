import { Circle, Point, is_inside, is_inside_move } from "./math.rs";

console.log(
  is_inside(
    {
      radius: 1,
      center: {
        x: 0,
        y: 1,
      },
    },
    {
      x: 0,
      y: 1,
    },
  ),
);
