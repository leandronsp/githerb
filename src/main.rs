//! githerb: argument parsing and wiring, and nothing else.
//!
//! Every command reads the same three things out of the repository, calls one
//! use case and prints what it answered. Nothing is decided here: a rule that
//! turns up in this file belongs in `app` or in the core.
//!
//! Exit codes: 0 when it worked, 2 when the command line does not say what to
//! do, and 1 for everything else, with the refusal on stderr.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod cli;

use std::io::{self, Read, Write};

use app::{Config, Identity, Reader, Result, Store, format};
use clap::Parser;
use review::{Anchor, Author, FilePath, ProposalId, RecordId, Side, Span};

use crate::cli::{Cli, Command};

fn main() {
    if let Err(err) = run(&Cli::parse().command) {
        let _ignored = writeln!(io::stderr(), "githerb: {err}");
        std::process::exit(1);
    }
}

/// What the repository is, who is working in it and what it declares.
struct Session {
    store: Store,
    config: Config,
    author: Author,
}

impl Session {
    fn open() -> Result<Session> {
        let store = Store::at(".")?;
        let config = Config::load(store.repo().root())?;
        let author = Identity::detect(store.repo());

        Ok(Session {
            store,
            config,
            author,
        })
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per verb, each of them three lines: splitting it would only \
              scatter the same list across files"
)]
fn run(command: &Command) -> Result<()> {
    let mut out = io::stdout().lock();

    match command {
        Command::Version => writeln!(out, "githerb {}", env!("CARGO_PKG_VERSION"))?,

        Command::Propose {
            revision,
            onto,
            title,
        } => {
            let session = Session::open()?;
            let proposal = app::propose(
                &session.store,
                &session.author,
                app::now(),
                title,
                onto,
                revision,
            )?;

            writeln!(
                out,
                "{}  revision 1  onto {}",
                proposal.id(),
                proposal.target()
            )?;
        }

        Command::List => {
            let session = Session::open()?;

            write!(out, "{}", format::list(&session.store.list()?))?;
        }

        Command::Show { proposal } => {
            let session = Session::open()?;

            write!(
                out,
                "{}",
                format::show(&session.store.load(&id(proposal)?)?)
            )?;
        }

        Command::Diff { proposal, since } => {
            let session = Session::open()?;

            writeln!(
                out,
                "{}",
                app::diff(&session.store, &id(proposal)?, *since)?
            )?;
        }

        Command::Comment {
            proposal,
            file,
            line,
            side,
            body,
        } => {
            let session = Session::open()?;
            let anchor = Anchor::new(
                FilePath::parse(file)?,
                Span::new(Side::parse(side)?, line.start, line.end)?,
            );
            let comment = app::annotate(
                &session.store,
                &session.author,
                app::now(),
                &id(proposal)?,
                anchor,
                body,
            )?;

            writeln!(out, "{}", comment.id())?;
        }

        Command::Comments {
            proposal,
            json,
            all,
        } => {
            let session = Session::open()?;
            let scope = if *all {
                format::Scope::All
            } else {
                format::Scope::Open
            };
            let shape = if *json {
                format::Shape::Json
            } else {
                format::Shape::Text
            };

            write!(
                out,
                "{}",
                format::comments(&session.store.load(&id(proposal)?)?, scope, shape)
            )?;
        }

        Command::Reply {
            proposal,
            comment,
            body,
        } => {
            let session = Session::open()?;
            let answer = app::reply(
                &session.store,
                &session.author,
                app::now(),
                &id(proposal)?,
                &note(comment)?,
                body,
            )?;

            writeln!(out, "{}", answer.id())?;
        }

        Command::Resolve { proposal, comment } => {
            let session = Session::open()?;

            app::resolve(
                &session.store,
                &session.author,
                app::now(),
                &id(proposal)?,
                &note(comment)?,
            )?;
        }

        Command::Handover { proposal, agent } => {
            let session = Session::open()?;
            let reader = if *agent {
                Reader::Agent
            } else {
                Reader::Person
            };
            let brief = app::handover(&session.store, &id(proposal)?, reader)?;

            if brief.is_empty() {
                writeln!(out, "nothing open")?;
            } else {
                write!(out, "{brief}")?;
            }
        }

        Command::Work {
            phase,
            proposal,
            task,
            note,
        } => {
            let session = Session::open()?;
            let line = app::report(
                &session.store,
                &session.author,
                app::now(),
                &id(proposal)?,
                task,
                phase.recorded(),
                note.as_deref(),
            )?;

            writeln!(out, "{} {} {}", line.agent(), line.task(), line.phase())?;
        }

        Command::Dispatch { proposal } => {
            let session = Session::open()?;
            let asked = app::dispatch(&session.store, &session.author, app::now(), &id(proposal)?)?;

            writeln!(
                out,
                "{} handed over with {} open",
                asked.id(),
                asked.open_comments().len()
            )?;
        }

        Command::Revise { proposal, revision } => {
            let session = Session::open()?;
            let revised = app::revise(&session.store, &id(proposal)?, revision)?;

            writeln!(
                out,
                "{}  revision {}",
                revised.id(),
                revised.head().number()
            )?;
        }

        Command::Describe { proposal, template } => {
            if *template {
                write!(out, "{}", app::template())?;

                return Ok(());
            }

            let session = Session::open()?;
            let mut json = String::new();
            io::stdin().read_to_string(&mut json)?;

            let written = app::describe(
                &session.store,
                &session.author,
                app::now(),
                &id(proposal.as_deref().unwrap_or_default())?,
                &json,
            )?;

            writeln!(out, "{written} written")?;
        }

        Command::Check { proposal } => {
            let session = Session::open()?;
            let results = app::check(
                &session.store,
                &session.config,
                &session.author,
                app::now(),
                &id(proposal)?,
                &mut out,
            )?;
            let failed = app::refused(&results);

            if failed > 0 {
                return Err(app::Error::CheckFailed {
                    failed,
                    total: results.len(),
                });
            }
        }

        Command::Land { proposal } => {
            let session = Session::open()?;
            let landing = app::land(
                &session.store,
                &app::required(&session.config)?,
                &session.author,
                app::now(),
                &id(proposal)?,
            )?;
            let landed = landing.proposal();

            writeln!(
                out,
                "{} landed onto {} at {}",
                landed.id(),
                landed.target(),
                landed.head().sha().short()
            )?;

            for followed in landing.followed() {
                writeln!(out, "{followed} now lands onto {}", landed.target())?;
            }
        }

        Command::Abandon { proposal } => {
            let session = Session::open()?;
            let given_up =
                app::abandon(&session.store, &session.author, app::now(), &id(proposal)?)?;

            writeln!(out, "{} abandoned", given_up.id())?;
        }
    }

    Ok(())
}

/// A proposal named on the command line.
fn id(raw: &str) -> Result<ProposalId> {
    Ok(ProposalId::parse(raw)?)
}

/// A note named on the command line.
fn note(raw: &str) -> Result<RecordId> {
    Ok(RecordId::parse(raw)?)
}
