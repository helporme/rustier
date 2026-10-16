pub struct Receipt {
    pub number: i32,
}

impl Receipt {
    pub fn label(&self) -> String {
        self.number.to_string()
    }
}

pub enum Status {
    Ready,
    Pending { position: i32 },
}

#[derive(Clone)]
pub struct Meter<T> {
    pub value: T,
}

impl<T: Clone> Meter<T> {
    pub fn accepted(value: T) -> Result<T, String> {
        Ok(value)
    }

    pub fn read(&self) -> Result<T, String> {
        Ok(self.value.clone())
    }

    pub fn replace(&mut self, value: T) -> Result<T, String> {
        Ok(std::mem::replace(&mut self.value, value))
    }
}

#[derive(Clone)]
pub struct Report {
    pub outcome: Result<i32, String>,
}

type ReadingResult<T> = Result<T, String>;

pub fn read_number() -> Result<i32, String> {
    Ok(42)
}

pub fn read_text() -> Result<String, i32> {
    Ok("ready".to_owned())
}

pub fn read_flag() -> Result<bool, String> {
    Ok(false)
}

pub fn save() -> Result<(), String> {
    Ok(())
}

pub fn issue_receipt(number: i32) -> Result<Receipt, String> {
    Ok(Receipt { number })
}

pub fn read_status(pending: bool) -> Result<Status, String> {
    if pending {
        Ok(Status::Pending { position: 3 })
    } else {
        Ok(Status::Ready)
    }
}

pub fn aliased_reading() -> ReadingResult<i32> {
    Ok(17)
}

pub fn make_meter(value: i32) -> Meter<i32> {
    Meter { value }
}

pub fn describe_outcome(outcome: Result<i32, String>) -> String {
    match outcome {
        Ok(value) => value.to_string(),
        Err(error) => error,
    }
}

pub fn nested_outcome() -> Result<Result<i32, String>, String> {
    Ok(Err("pending".to_owned()))
}

pub fn make_report() -> Report {
    Report { outcome: Ok(8) }
}
