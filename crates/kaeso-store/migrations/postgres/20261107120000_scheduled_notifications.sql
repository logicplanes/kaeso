DELETE FROM notification_settings WHERE kind = 'cost_report' AND delivery = 'daily';

UPDATE notification_settings SET delivery = 'now', updated_at = now()
WHERE kind = 'weekly_summary' AND delivery = 'daily';
