output "url" {
  description = "Where Kaeso answers once the certificate is issued, a minute or two after the instance is up."
  value       = "https://${var.domain}"
}

output "public_ip" {
  description = "The instance's address; point the domain's A record at it when no DNS zone was given."
  value       = google_compute_address.kaeso.address
}

output "setup_link" {
  description = "Prints the one-time setup link; the install's folder is /opt/kaeso."
  value       = "gcloud compute ssh ${var.name} --zone ${var.zone} -- 'cd /opt/kaeso && sudo docker compose exec gateway kaeso setup-link'"
}
