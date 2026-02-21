output "instance_id" {
  description = "Enclave parent host instance ID."
  value       = aws_instance.this.id
}

output "private_ip" {
  description = "Enclave parent host private IP."
  value       = aws_instance.this.private_ip
}

output "availability_zone" {
  description = "Availability zone used by enclave parent host."
  value       = aws_instance.this.availability_zone
}
