output "url" {
  description = "Where Kaeso answers once the certificate is issued, which takes up to an hour after DNS points at the address."
  value       = "https://${var.domain}"
}

output "edge_ip" {
  description = "The load balancer's address; point the domain's A record at it when no DNS zone was given."
  value       = google_compute_global_address.edge.address
}

output "master_keys_secret" {
  description = "The Secret Manager secret holding the master keys; back it up outside Google Cloud."
  value       = google_secret_manager_secret.secrets["master-keys"].name
}
