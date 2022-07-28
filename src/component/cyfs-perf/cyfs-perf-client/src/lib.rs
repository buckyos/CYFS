mod isolate;
mod config;
mod store;
mod manager;

pub use config::*;
pub use isolate::*;

#[macro_use]
extern crate log;

#[cfg(test)]
mod tests {
    fn search_lastsmall<E: PartialOrd>(data: Vec<E>, target: E) -> i32 {
        if data.len() <= 1 {
            return 0;
        }
        let mut l: i32 = 0;
        // 左闭右开区间
        let mut r = data.len() as i32 -1;
        while l <= r {
            let mid = (l + r) / 2;
            if data[mid as usize] <= target {
                if mid == (data.len() -1) as i32 || data[mid as usize + 1] > target {
                    return mid;
                }
                l = mid + 1;
            } else {
                r = mid - 1;
            }
        }

        return 0;
    }

    #[test]
    fn it_works() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 11];
        assert_eq!(search_lastsmall(data.to_owned(), 11), 12);
        assert_eq!(search_lastsmall(data.to_owned(), 10), 10);
        assert_eq!(search_lastsmall(data.to_owned(), 9), 9);
        assert_eq!(search_lastsmall(data.to_owned(), 8), 8);
        assert_eq!(search_lastsmall(data.to_owned(), 7), 7);
        assert_eq!(search_lastsmall(data.to_owned(), 6), 6);
        assert_eq!(search_lastsmall(data.to_owned(), 5), 5);
        assert_eq!(search_lastsmall(data.to_owned(), 4), 4);
        assert_eq!(search_lastsmall(data.to_owned(), 3), 3);
        assert_eq!(search_lastsmall(data.to_owned(), 2), 2);
        assert_eq!(search_lastsmall(data.to_owned(), 1), 1);
        assert_eq!(search_lastsmall(data.to_owned(), 0), 0);
    }
}