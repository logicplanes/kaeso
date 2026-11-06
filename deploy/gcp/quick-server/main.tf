terraform {
  required_version = ">= 1.6"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }
}

provider "google" {
  project = var.project
  region  = var.region
  zone    = var.zone
}

resource "google_compute_address" "kaeso" {
  name = "${var.name}-address"
}

resource "google_compute_firewall" "web" {
  name    = "${var.name}-web"
  network = "default"
  allow {
    protocol = "tcp"
    ports    = ["80", "443"]
  }
  allow {
    protocol = "udp"
    ports    = ["443"]
  }
  source_ranges = ["0.0.0.0/0"]
  target_tags   = [var.name]
}

resource "google_compute_firewall" "ssh" {
  name    = "${var.name}-ssh"
  network = "default"
  allow {
    protocol = "tcp"
    ports    = ["22"]
  }
  source_ranges = var.ssh_source_ranges
  target_tags   = [var.name]
}

resource "google_compute_instance" "kaeso" {
  name         = var.name
  machine_type = var.machine_type
  tags         = [var.name]

  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2404-lts-amd64"
      size  = var.disk_size_gb
      type  = "pd-balanced"
    }
  }

  network_interface {
    network = "default"
    access_config {
      nat_ip = google_compute_address.kaeso.address
    }
  }

  shielded_instance_config {
    enable_secure_boot = true
  }

  metadata = {
    enable-oslogin = "TRUE"
  }

  metadata_startup_script = <<-EOT
    #!/bin/bash
    set -euo pipefail
    if [ -f /opt/kaeso/compose.yaml ]; then exit 0; fi
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -q
    apt-get install -y -q curl ca-certificates
    curl -fsSL https://kaeso.app/install.sh | sh -s -- --domain "${var.domain}" --dir /opt/kaeso --install-docker ${var.kaeso_version == "" ? "" : "--version ${var.kaeso_version}"}
  EOT
}

resource "google_dns_record_set" "kaeso" {
  count        = var.dns_zone == "" ? 0 : 1
  managed_zone = var.dns_zone
  name         = "${var.domain}."
  type         = "A"
  ttl          = 300
  rrdatas      = [google_compute_address.kaeso.address]
}
