pub(super) const STRING: &str = r#"module receiving;
@id("app.probe") fn probe()->i64 {
 let key=str"x";let mut text="a";let mut at=0;
 while at<3 {at=at+1;text=string_concat(text,"b");if at==1 {continue;} else {0} if at==3 {return string_len(text)+str_len_bytes(key);} else {0} 0}
 string_len(text)
}
@id("app.main") fn main()->i64 {probe()}
"#;
pub(super) const VECTOR: &str = r#"module receiving;
@id("app.probe") fn probe()->i64 {
 let key=str"x";let mut rows=vec_with_capacity<i64>(3usize);let mut at=0;
 while at<3 {at=at+1;rows=vec_push<i64>(rows,7);if at==1 {continue;} else {0} if at==3 {return i64_from_usize(vec_len<i64>(rows))+str_len_bytes(key);} else {0} 0}
 0
}
@id("app.main") fn main()->i64 {probe()}
"#;
pub(super) const RECORD: &str = r#"module receiving;
@id("app.Row") record Row {@id("app.Row.n") n:i64,@id("app.Row.text") text:string,}
@id("app.probe") fn probe(rows:borrow Vec<Row>)->i64 {
 let key=str"x";let mut seen=0;
 for borrow row at index in rows {seen=seen+row.n;if index==0usize {continue;} else {0} let text=row.text;if str_len_bytes(text)==str_len_bytes(key) {return seen;} else {0} 0}
 seen
}
@id("app.main") fn main()->i64 {let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:3,text:"x"});let rows=vec_push<Row>(first,Row{n:7,text:"x"});probe(rows)}
"#;
