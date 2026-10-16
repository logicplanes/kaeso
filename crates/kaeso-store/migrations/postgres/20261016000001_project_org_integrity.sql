ALTER TABLE api_keys
    ADD CONSTRAINT api_keys_project_org FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE;
ALTER TABLE catalogue_models
    ADD CONSTRAINT catalogue_models_project_org FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE;
ALTER TABLE aliases
    ADD CONSTRAINT aliases_project_org FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE;
ALTER TABLE limits
    ADD CONSTRAINT limits_project_org FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE;
ALTER TABLE log_jobs
    ADD CONSTRAINT log_jobs_project_org FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE;
ALTER TABLE usage_daily
    ADD CONSTRAINT usage_daily_project_org FOREIGN KEY (project_id, org_id) REFERENCES projects (id, org_id) ON DELETE CASCADE;

CREATE INDEX api_keys_org ON api_keys (org_id);
