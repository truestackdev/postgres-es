use std::rc::Rc;
use std::sync::RwLock;

use cqrs_es::{Aggregate, AggregateError, Command, DomainEvent, EventStore, MessageEnvelope};
use cqrs_es::view::ViewProcessor;
use postgres::{Connection, TlsMode};
use serde::{Deserialize, Serialize};
use postgres_es::PostgresStore;
use static_assertions::assert_impl_all;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestAggregate {
    id: String,
    description: String,
    tests: Vec<String>,
}

impl Aggregate for TestAggregate { fn aggregate_type() -> &'static str { "TestAggregate" } }

impl Default for TestAggregate {
    fn default() -> Self {
        TestAggregate {
            id: "".to_string(),
            description: "".to_string(),
            tests: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TestEvent {
    Created(Created),
    Tested(Tested),
    SomethingElse(SomethingElse),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Created {
    pub id: String
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Tested {
    pub test_name: String
}

impl DomainEvent<TestAggregate> for Tested {
    fn apply(self, aggregate: &mut TestAggregate) {
        aggregate.tests.push(self.test_name);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SomethingElse {
    pub description: String
}

impl DomainEvent<TestAggregate> for TestEvent {
    fn apply(self, aggregate: &mut TestAggregate) {
        match self {
            TestEvent::Created(e) => {
                aggregate.id = e.id;
            }
            TestEvent::Tested(e) => { e.apply(aggregate) }
            TestEvent::SomethingElse(e) => {
                aggregate.description = e.description;
            }
        }
    }
}

pub struct CreateTest {
    pub id: String,
}

impl Command<TestAggregate, TestEvent> for CreateTest {
    fn handle(self, _aggregate: &TestAggregate) -> Result<Vec<TestEvent>, AggregateError> {
        let event = TestEvent::Created(Created { id: self.id.to_string() });
        Ok(vec![event])
    }
}

pub struct ConfirmTest<'a> {
    pub test_name: &'a str,
}

impl<'a> Command<TestAggregate, TestEvent> for ConfirmTest<'a> {
    fn handle(self, aggregate: &TestAggregate) -> Result<Vec<TestEvent>, AggregateError> {
        for test in &aggregate.tests {
            if test == &self.test_name {
                return Err(AggregateError::new("test already performed"));
            }
        }
        let event = TestEvent::Tested(Tested { test_name: self.test_name.to_string() });
        Ok(vec![event])
    }
}

pub struct DoSomethingElse {
    pub description: String,
}

impl Command<TestAggregate, TestEvent> for DoSomethingElse {
    fn handle(self, _aggregate: &TestAggregate) -> Result<Vec<TestEvent>, AggregateError> {
        let event = TestEvent::SomethingElse(SomethingElse { description: self.description.clone() });
        Ok(vec![event])
    }
}


struct TestView {
    events: Rc<RwLock<Vec<MessageEnvelope<TestAggregate, TestEvent>>>>
}

impl TestView {
    fn new(events: Rc<RwLock<Vec<MessageEnvelope<TestAggregate, TestEvent>>>>) -> Self { TestView { events } }
}


impl ViewProcessor<TestAggregate, TestEvent> for TestView {
    fn dispatch(&self, _aggregate_id: &str, events: Vec<MessageEnvelope<TestAggregate, TestEvent>>) {
        for event in events {
            let mut event_list = self.events.write().unwrap();
            event_list.push(event);
        }
    }
}

pub type TestMessageEnvelope = MessageEnvelope<TestAggregate, TestEvent>;


assert_impl_all!(rdbmsstore; PostgresStore::<TestAggregate,TestEvent>, EventStore::<TestAggregate,TestEvent>);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;

    use postgres_es::PostgresStore;

    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    fn metadata() -> HashMap<String, String> {
        let now = Utc::now();
        let mut metadata = HashMap::new();
        metadata.insert("time".to_string(), now.to_rfc3339());
        metadata
    }

    #[test]
    #[ignore] // integration testing
    fn commit_and_load_events() {
        let conn = Connection::connect("postgresql://user:pass@localhost:5432/test_db", TlsMode::None).unwrap();
        let event_store = PostgresStore::<TestAggregate, TestEvent>::new(conn);
        let id = uuid::Uuid::new_v4().to_string();
        let aggregate_type = "TestAggregate".to_string();

        let events = event_store.load(&id);
        assert_eq!(0, events.len());

        event_store.commit(vec![
            TestMessageEnvelope::new_with_metadata(
                id.clone(),
                0,
                aggregate_type.clone(),
                TestEvent::Created(Created { id: "test_event_A".to_string() }),
                metadata(),
            ),
            TestMessageEnvelope::new_with_metadata(
                id.clone(),
                1,
                aggregate_type.clone(),
                TestEvent::Tested(Tested { test_name: "test A".to_string() }),
                metadata()),
        ]);

        let events = event_store.load(&id);
        assert_eq!(2, events.len());
    }

    #[test]
    #[ignore] // integration testing
    fn new_command() {
        let conn = Connection::connect("postgresql://user:pass@localhost:5432/test_db", TlsMode::None).unwrap();
        let event_store = PostgresStore::<TestAggregate, TestEvent>::new(conn);
        let id = uuid::Uuid::new_v4().to_string();
        let id_str = id.to_string();
        let aggregate_type = "TestAggregate".to_string();

        let events = event_store.load(&id);
        assert_eq!(0, events.len());

        event_store.commit(vec![
            TestMessageEnvelope::new_with_metadata(
                id_str.clone(),
                0,
                aggregate_type.clone(),
                TestEvent::Created(Created { id: "test_event_A".to_string() }),
                metadata(),
            ),
            TestMessageEnvelope::new_with_metadata(
                id_str.clone(),
                1,
                aggregate_type.clone(),
                TestEvent::Tested(Tested { test_name: "test A".to_string() }),
                metadata()),
        ]);

        let events = event_store.load(&id);
        assert_eq!(2, events.len());
    }
}