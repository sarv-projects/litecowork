import json
import unittest
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = json.loads((ROOT / "docs/schemas/operator-stream.schema.json").read_text(encoding="utf-8"))


class OperatorStreamContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.frames = jsonschema.Draft202012Validator(SCHEMA)
        cls.subscribe = jsonschema.Draft202012Validator(SCHEMA["$defs"]["subscribe"])

    def test_subscribe_is_bounded_and_closed(self):
        valid = {
            "workspace_id": "workspace-1",
            "conversation_ids": ["conversation-1"],
            "projection_types": ["conversation_presentation"],
        }
        self.assertTrue(self.subscribe.is_valid(valid))
        self.assertFalse(self.subscribe.is_valid({**valid, "bearer_token": "secret"}))
        self.assertFalse(self.subscribe.is_valid({**valid, "conversation_ids": ["same", "same"]}))
        self.assertFalse(self.subscribe.is_valid({**valid, "projection_types": []}))
        runtime_inventory = {
            "workspace_id": "workspace-1",
            "runtime_ids": ["runtime-1"],
            "projection_types": ["runtime_inventory"],
        }
        self.assertTrue(self.subscribe.is_valid(runtime_inventory))
        self.assertFalse(self.subscribe.is_valid({**runtime_inventory, "runtime_ids": ["r"] * 9}))

    def test_multi_conversation_resume_cursor_bound_is_supported_by_all_stream_frames(self):
        maximum_cursor = "c" * 14_336
        subscribe = {
            "workspace_id": "workspace-1",
            "conversation_ids": [f'conversation-"{index}' for index in range(20)],
            "projection_types": ["conversation_presentation"],
            "resume_cursor": maximum_cursor,
        }
        self.assertTrue(self.subscribe.is_valid(subscribe))
        self.assertFalse(self.subscribe.is_valid({**subscribe, "resume_cursor": maximum_cursor + "c"}))

        ready = {
            "type": "stream.ready",
            "workspace_id": "workspace-1",
            "projection_version": 1,
            "cursor": maximum_cursor,
            "max_frame_bytes": 1_048_576,
        }
        self.assertTrue(self.frames.is_valid(ready))
        self.assertFalse(self.frames.is_valid({**ready, "cursor": maximum_cursor + "c"}))

        projection = {
            "type": "projection",
            "workspace_id": "workspace-1",
            "projection_type": "conversation_presentation",
            "projection_revision": 1,
            "cursor": maximum_cursor,
            "payload": {
                "conversation_id": "conversation-1",
                "stream_cursor": "conversation:1",
                "origin_sequence": "1",
                "items": [],
                "next_cursor": None,
                "active_turn": None,
            },
        }
        self.assertTrue(self.frames.is_valid(projection))
        self.assertFalse(self.frames.is_valid({**projection, "cursor": maximum_cursor + "c"}))

    def test_stream_frame_fixtures_match_closed_wire_shapes(self):
        ready = {
            "type": "stream.ready", "workspace_id": "workspace-1", "projection_version": 1,
            "cursor": "cursor-1", "max_frame_bytes": 1024,
        }
        linked_item = {
            "item_key": "message-1-text", "kind": "TEXT", "payload_version": 1,
            "source_refs": [{"kind": "MESSAGE", "id": "message-1"}],
            "order_key": "001", "freshness": "CURRENT", "payload": {"text": "Hello"},
        }
        message = {
            "message": {
                "message_id": "message-1", "conversation_id": "conversation-1",
                "author": {"principal_id": "principal-1", "kind": "USER"},
                "role": "USER", "content": [{"kind": "TEXT", "text": "Hello"}],
                "resource_refs": [], "created_at": "2026-10-11T10:00:00Z",
            },
            "rich_presentation": None, "linked_items": [linked_item],
        }
        snapshot = {
            "type": "projection", "workspace_id": "workspace-1",
            "projection_type": "conversation_presentation", "projection_revision": 1,
            "cursor": "cursor-2", "payload": {
                "conversation_id": "conversation-1", "stream_cursor": "conversation:1",
                "origin_sequence": "9", "items": [message], "next_cursor": None,
                "active_turn": None,
            },
        }
        upsert = {
            **snapshot, "projection_revision": 2, "cursor": "cursor-3",
            "payload": {
                "conversation_id": "conversation-1", "stream_cursor": "conversation:2",
                "origin_sequence": "10", "item": message,
            },
        }
        self.assertTrue(self.frames.is_valid(ready))
        self.assertTrue(self.frames.is_valid(snapshot))
        self.assertTrue(self.frames.is_valid(upsert))
        malformed_payload = {**snapshot["payload"], "items": [message, {"message": {"role": "USER"}}]}
        malformed = {**snapshot, "payload": malformed_payload}
        self.assertFalse(self.frames.is_valid(malformed))
        self.assertFalse(self.frames.is_valid({**snapshot, "payload": {**snapshot["payload"], "extra": True}}))
        self.assertFalse(self.frames.is_valid({**ready, "untrusted": True}))

    def test_task_projection_frames_use_a_closed_snapshot_with_a_durable_frontier(self):
        task_item = {
            "item_key": "task:task-1",
            "kind": "TASK_CARD",
            "payload_version": 1,
            "source_refs": [{"kind": "TASK", "id": "task-1", "revision": "1"}],
            "occurred_at": "2026-10-11T10:00:00Z",
            "order_key": "0:task-1",
            "status": "WAITING",
            "label": "Inspect the report",
            "payload": {
                "task_id": "task-1",
                "objective": "Inspect the report",
                "status": "READY",
            },
            "freshness": "CURRENT",
        }
        frame = {
            "type": "projection",
            "workspace_id": "workspace-1",
            "projection_type": "task_presentation",
            "projection_revision": 2,
            "cursor": "opaque-task-cursor",
            "payload": {
                "workspace_id": "workspace-1",
                "task_id": "task-1",
                "origin_sequence": "2",
                "computed_at": "2026-10-11T10:00:00Z",
                "freshness": "CURRENT",
                "items": [task_item],
            },
        }
        self.assertTrue(self.frames.is_valid(frame))
        malformed = {
            **frame,
            "payload": {**frame["payload"], "private_provider_handle": "must-not-cross"},
        }
        self.assertFalse(self.frames.is_valid(malformed))
        self.assertFalse(self.frames.is_valid({
            **frame,
            "payload": {**frame["payload"], "origin_sequence": "02"},
        }))

    def test_runtime_inventory_notifications_are_closed_and_invalidation_only(self):
        frame = {
            "type": "projection",
            "workspace_id": "workspace-1",
            "projection_type": "runtime_inventory",
            "projection_revision": 4,
            "cursor": "opaque-runtime-cursor",
            "payload": {"runtime_id": "runtime-1", "change_kind": "OFFERS_CHANGED"},
        }
        self.assertTrue(self.frames.is_valid(frame))
        self.assertFalse(self.frames.is_valid({
            **frame,
            "payload": {**frame["payload"], "endpoint_locator": "/private/socket"},
        }))
        self.assertFalse(self.frames.is_valid({
            **frame,
            "payload": {"runtime_id": "runtime-1", "change_kind": "PROVIDER_HANDLE"},
        }))


if __name__ == "__main__":
    unittest.main()
