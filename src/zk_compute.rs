use pbc_zk::*;

pub fn zk_compute() -> (Vec<u128>) {
    let mut votes: Vec<u128> = vec![];
    for variable_id in 1..(num_secret_variables() + 1) {
        if sbi32_input(variable_id) >= sbi32_from(0) && sbi32_input(variable_id) <= sbi32_from(500) {
            // votes[sbi32_input(variable_id)] =  votes[sbi32_input(variable_id)]+1;
        }
    }

    votes
}
