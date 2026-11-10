#!/usr/bin/env python3
"""
Unit and integration tests for Antigravity Account Switcher.
Tests strictly target Antigravity standalone environment, profile serialization,
dual-write file fallback, rate-limit cooldowns, and auto-switch behavior.
"""

import unittest
import tempfile
import os
import shutil
import json
import time
from pathlib import Path

# Import the switcher functions
import antigravity_switcher as sw

class TestAntigravitySwitcher(unittest.TestCase):
    def setUp(self):
        self.test_dir = Path(tempfile.mkdtemp(prefix="antigravity_test_"))
        
        # Override config paths for testing
        sw.get_config_dir = lambda: self.test_dir / "config"
        sw.get_profiles_path = lambda: self.test_dir / "config" / "profiles.json"
        sw.get_settings_path = lambda: self.test_dir / "config" / "settings.json"
        sw.get_gemini_dir = lambda: self.test_dir / ".gemini"
        sw.get_jetski_token_file = lambda: self.test_dir / ".gemini" / "jetski-standalone-oauth-token"
        sw.get_antigravity_cli_token_file = lambda: self.test_dir / ".gemini" / "antigravity-cli" / "antigravity-oauth-token"
        sw.get_oauth_creds_file = lambda: self.test_dir / ".gemini" / "oauth_creds.json"
        sw.get_google_accounts_file = lambda: self.test_dir / ".gemini" / "google_accounts.json"
        sw.get_antigravity_storage_dir = lambda: self.test_dir / ".config" / "Antigravity"
        sw.find_app_storage = lambda: self.test_dir / ".config" / "Antigravity" / "app_storage.json"
        sw.find_antigravity_db = lambda: None
        sw.kill_language_server_processes = lambda: 1  # Mock successful hot switch

        (self.test_dir / "config").mkdir(parents=True, exist_ok=True)
        (self.test_dir / ".gemini").mkdir(parents=True, exist_ok=True)
        (self.test_dir / ".config" / "Antigravity").mkdir(parents=True, exist_ok=True)

        # Mock initial app_storage.json
        storage_data = {
            "agy.onboarding.lastLoginUsername": "developer.one@gmail.com",
            "agy.onboarding.lastLoginIsGcpTos": False
        }
        with open(sw.find_app_storage(), 'w', encoding='utf-8') as f:
            json.dump(storage_data, f)

        # Mock initial jetski token file
        token_data = {
            "token": {
                "access_token": "mock-access-token-123",
                "token_type": "Bearer",
                "refresh_token": "mock-refresh-token-456",
                "expiry": "2026-11-10T22:00:00.000000Z",
                "expires_in": 3599
            },
            "auth_method": "consumer",
            "id_token_email": "developer.one@gmail.com"
        }
        with open(sw.get_jetski_token_file(), 'w', encoding='utf-8') as f:
            json.dump(token_data, f)

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_get_active_session(self):
        session = sw.get_active_session()
        self.assertEqual(session["email"], "developer.one@gmail.com")
        self.assertEqual(session["name"], "developer.one")
        self.assertTrue(session["has_access_token"])
        self.assertTrue(session["has_refresh_token"])

    def test_save_and_list_profiles(self):
        p1 = sw.save_current_account("Primary Dev Account")
        self.assertEqual(p1["label"], "Primary Dev Account")
        self.assertEqual(p1["email"], "developer.one@gmail.com")
        self.assertTrue(p1["is_active"])

        profiles = sw.load_profiles()
        self.assertEqual(len(profiles), 1)
        self.assertEqual(profiles[0]["id"], p1["id"])

    def test_mark_exhausted_and_cooldown(self):
        p1 = sw.save_current_account("Account 1")
        marked = sw.mark_exhausted(p1["id"], cooldown_minutes=120)
        self.assertTrue(marked["is_exhausted"])
        self.assertIsNotNone(marked["exhausted_until"])
        self.assertTrue(marked["exhausted_until"] > int(time.time()))

    def test_hot_switch_and_dual_write(self):
        p1 = sw.save_current_account("Account 1")

        # Create Profile 2
        profiles = sw.load_profiles()
        p2 = {
            'id': 'profile-2-uuid',
            'label': 'Backup Account',
            'email': 'developer.two@gmail.com',
            'name': 'developer.two',
            'tier': 'Pro',
            'picture': None,
            'access_token': 'mock-access-token-789',
            'refresh_token': 'mock-refresh-token-999',
            'id_token': None,
            'is_active': False,
            'is_exhausted': False,
            'exhausted_until': None,
            'last_used': 0,
            'created_at': int(time.time())
        }
        profiles.append(p2)
        sw.save_profiles(profiles)

        # Switch to Profile 2
        switched = sw.switch_account("profile-2-uuid")
        self.assertEqual(switched["email"], "developer.two@gmail.com")
        self.assertTrue(switched["hot_switched"])

        # Verify dual-write to files
        with open(sw.get_jetski_token_file(), 'r') as f:
            jetski_content = json.load(f)
            self.assertEqual(jetski_content["id_token_email"], "developer.two@gmail.com")
            self.assertEqual(jetski_content["token"]["access_token"], "mock-access-token-789")

        with open(sw.find_app_storage(), 'r') as f:
            storage_content = json.load(f)
            self.assertEqual(storage_content["agy.onboarding.lastLoginUsername"], "developer.two@gmail.com")

    def test_auto_switch_next(self):
        p1 = sw.save_current_account("Account 1")

        # Create Profile 2
        profiles = sw.load_profiles()
        p2 = dict(p1)
        p2["id"] = "test-id-2"
        p2["label"] = "Account 2"
        p2["email"] = "developer.two@gmail.com"
        p2["is_active"] = False
        p2["is_exhausted"] = False
        p2["exhausted_until"] = None
        profiles.append(p2)
        sw.save_profiles(profiles)

        # Mark Account 1 exhausted
        sw.mark_exhausted(p1["id"], cooldown_minutes=300)

        # Trigger auto-switch
        switched = sw.auto_switch_next()
        self.assertEqual(switched["id"], "test-id-2")
        self.assertEqual(switched["email"], "developer.two@gmail.com")
        self.assertTrue(switched["is_active"])

if __name__ == '__main__':
    unittest.main()
