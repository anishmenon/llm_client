pub mod round;
pub mod step;

use crate::components::base_request::BaseLlmRequest;
use anyhow::{anyhow, Result};
use core::panic;
pub use round::CascadeRound;

#[derive(Clone)]
pub struct CascadeFlow {
    pub cascade_name: String,
    pub duration: std::time::Duration,
    pub result_can_be_none: bool,
    pub rounds: Vec<CascadeRound>,
    pub start_time: std::time::Instant,
}

impl CascadeFlow {
    pub fn new<T: Into<String>>(cascade_name: T) -> Self {
        Self {
            cascade_name: cascade_name.into(),
            start_time: std::time::Instant::now(),
            duration: std::time::Duration::default(),
            rounds: Vec::new(),
            result_can_be_none: false,
        }
    }

    pub fn new_round<T: Into<String>>(&mut self, task: T) -> &mut CascadeRound {
        let round = CascadeRound::new(task);
        self.rounds.push(round);
        self.rounds.last_mut().unwrap()
    }

    pub async fn run_all_rounds(&mut self, base_req: &mut BaseLlmRequest) -> Result<()> {
        self.start_time = std::time::Instant::now();

        for round in self.rounds.iter_mut() {
            round.run_all_steps(base_req).await?;
        }
        base_req.instruct_prompt.prompt.build_final()?;
        self.duration = self.start_time.elapsed();
        Ok(())
    }

    pub fn last_round(&mut self) -> Result<&mut CascadeRound> {
        match self.rounds.last_mut() {
            Some(round) => Ok(round),
            None => Err(anyhow!("No rounds in cascade")),
        }
    }

    pub fn open_cascade(&mut self) {
        self.start_time = std::time::Instant::now();
    }

    pub fn close_cascade(&mut self, base_req: &mut BaseLlmRequest) -> Result<()> {
        base_req.instruct_prompt.prompt.build_final()?;
        self.duration = self.start_time.elapsed();
        Ok(())
    }

    pub fn primitive_result(&self) -> Option<String> {
        match self.rounds.last() {
            Some(round) => round.primitive_result(),
            None => panic!("No rounds in cascade"),
        }
    }
}

impl std::fmt::Display for CascadeFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        writeln!(f, "\x1b[1m\x1B[38;2;92;244;37m{}\x1b[0m", self.cascade_name)?;
        writeln!(f)?;
        for (i, round) in self.rounds.iter().enumerate() {
            let color = ROUND_GRADIENT[i % ROUND_GRADIENT.len()];
            writeln!(f, "\x1b[1m{color}Round {}\x1b[0m", i + 1)?;
            writeln!(f, "{round}",)?;
        }
        Ok(())
    }
}
static ROUND_GRADIENT: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
    vec![
        "\x1B[38;2;230;175;45m",
        "\x1B[38;2;235;158;57m",
        "\x1B[38;2;235;142;68m",
        "\x1B[38;2;232;127;80m",
        "\x1B[38;2;226;114;91m",
        "\x1B[38;2;216;103;100m",
        "\x1B[38;2;204;94;108m",
        "\x1B[38;2;189;88;114m",
        "\x1B[38;2;172;83;118m",
        "\x1B[38;2;153;79;119m",
        "\x1B[38;2;134;76;118m",
        "\x1B[38;2;115;73;114m",
        "\x1B[38;2;97;69;108m",
        "\x1B[38;2;80;65;99m",
        "\x1B[38;2;65;60;88m",
    ]
});
