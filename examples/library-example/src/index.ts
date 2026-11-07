import { rs } from 'rustier';

const vec = rs`vec![1,2,3]`;

rs`
  Vec::<i32>::push(${vec}, 4);
`;

rs`
  let list: &Vec<i32> = ${vec};
  for x in list {
    println!("{}", x);
  }
`
