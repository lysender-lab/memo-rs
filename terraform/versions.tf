terraform {
  required_version = "~> 1.0"

  backend "s3" {
    bucket       = "some-terraform-remote-backends"
    key          = "memo-rs-aws/terraform.tfstate"
    region       = "ap-southeast-1"
    use_lockfile = true
  }

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}
